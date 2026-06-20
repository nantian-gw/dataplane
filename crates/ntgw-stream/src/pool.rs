use std::sync::Arc;
use std::time::{Duration, Instant};

use dashmap::DashMap;
use tokio::net::TcpStream;
use tracing::debug;

use crate::traffic::StreamPoolCounters;

const MAX_CONNECTION_COUNT: usize = 32_768;

type PoolKey = (String, u16);

pub(crate) struct TcpConnectionPool {
	idle: Arc<DashMap<PoolKey, Vec<IdleConnection>>>,
	pub(super) max_idle_per_backend: usize,
	idle_timeout: Duration,
}

struct IdleConnection {
	stream: TcpStream,
	since: Instant,
}

impl TcpConnectionPool {
	pub(crate) fn new(max_idle_per_backend: usize, idle_timeout: Duration) -> Self {
		let max_idle_per_backend = max_idle_per_backend.clamp(0, MAX_CONNECTION_COUNT);
		Self {
			idle: Arc::new(DashMap::new()),
			max_idle_per_backend,
			idle_timeout,
		}
	}

	pub(crate) fn is_enabled(&self) -> bool {
		self.max_idle_per_backend > 0
	}

	pub(crate) async fn get_connection(
		&self,
		addr: &str,
		port: u16,
	) -> (std::io::Result<TcpStream>, StreamPoolCounters) {
		if !self.is_enabled() {
			let conn = TcpStream::connect((addr, port)).await;
			return (conn, StreamPoolCounters::default());
		}

		let key = (addr.to_string(), port);

		if let Some(mut entry) = self.idle.get_mut(&key) {
			while let Some(idle) = entry.pop() {
				if idle.since.elapsed() >= self.idle_timeout {
					debug!(backend = ?key, "pool eviction: idle timeout");
					continue;
				}

				let mut buf = [0u8; 1];
				match idle.stream.try_read(&mut buf) {
					Ok(0) => {
						debug!(backend = ?key, "pool eviction: stream closed");
						continue;
					}
					Ok(_) => {
						debug!(backend = ?key, "pool eviction: unexpected data pending");
						continue;
					}
					Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
						debug!(backend = ?key, "pool hit");
						return (Ok(idle.stream), StreamPoolCounters { hits: 1, misses: 0 });
					}
					Err(_) => {
						debug!(backend = ?key, "pool eviction: error");
						continue;
					}
				}
			}
		}

		debug!(backend = ?key, "pool miss");
		let conn = TcpStream::connect((addr, port)).await;
		(conn, StreamPoolCounters { hits: 0, misses: 1 })
	}

	pub(crate) fn return_connection(&self, addr: &str, port: u16, stream: TcpStream) {
		if !self.is_enabled() {
			return;
		}
		let key = (addr.to_string(), port);
		let mut entry = self.idle.entry(key).or_default();

		if entry.len() >= self.max_idle_per_backend {
			debug!(backend.addr = addr, backend.port = port, "pool full, dropping returned connection");
			return;
		}

		entry.push(IdleConnection {
			stream,
			since: Instant::now(),
		});
		debug!(backend.addr = addr, backend.port = port, count = entry.len(), "pool return");
	}

	pub(crate) fn drain(&self) {
		let count = self.idle.len();
		self.idle.clear();
		debug!(count, "pool drained");
	}

	#[cfg(test)]
	fn idle_count(&self, addr: &str, port: u16) -> usize {
		let key = (addr.to_string(), port);
		self.idle.get(&key).map_or(0, |e| e.len())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use tokio::net::TcpListener;

	async fn echo_server() -> (String, u16) {
		let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
		let addr = listener.local_addr().unwrap();
		let ip = addr.ip().to_string();
		let port = addr.port();
		tokio::spawn(async move {
			loop {
				let (socket, _) = listener.accept().await.unwrap();
				tokio::spawn(async move {
					let mut buf = [0; 1024];
					let _ = socket.try_read(&mut buf);
				});
			}
		});
		(ip, port)
	}

	#[tokio::test]
	async fn pool_returns_new_connection_when_empty() {
		let (ip, port) = echo_server().await;
		let pool = TcpConnectionPool::new(10, Duration::from_secs(30));

		let (result, counters) = pool.get_connection(&ip, port).await;
		assert!(result.is_ok(), "should connect to server");
		assert_eq!(counters.hits, 0);
		assert_eq!(counters.misses, 1);
	}

	#[tokio::test]
	async fn pool_reuses_returned_connection() {
		let (ip, port) = echo_server().await;
		let pool = TcpConnectionPool::new(10, Duration::from_secs(30));

		let (conn1, c1) = pool.get_connection(&ip, port).await;
		assert!(conn1.is_ok());
		assert_eq!(c1.misses, 1);

		pool.return_connection(&ip, port, conn1.unwrap());
		assert_eq!(pool.idle_count(&ip, port), 1);

		let (conn2, c2) = pool.get_connection(&ip, port).await;
		assert!(conn2.is_ok());
		assert_eq!(c2.hits, 1);
		assert_eq!(c2.misses, 0);
	}
}
