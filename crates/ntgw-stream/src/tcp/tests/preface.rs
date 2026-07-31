use anyhow::Result;
use tokio::{
    io::AsyncWriteExt,
    net::{TcpListener, TcpStream},
    time::{Duration, sleep, timeout},
};

use crate::sni::extract_server_name;

use super::*;

#[tokio::test]
async fn reads_preface_bytes() -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;

    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await?;
        read_preface(&mut stream).await
    });

    let mut client = TcpStream::connect(addr).await?;
    client.write_all(b"hello").await?;

    let payload = server.await??;
    assert_eq!(payload, b"hello");
    Ok(())
}

#[tokio::test]
async fn reads_fragmented_tls_client_hello() -> Result<()> {
    let hello = build_client_hello("abc.example.com");
    let split_at = 7;
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;

    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await?;
        read_preface(&mut stream).await
    });

    let mut client = TcpStream::connect(addr).await?;
    client.write_all(&hello[..split_at]).await?;
    sleep(Duration::from_millis(25)).await;
    client.write_all(&hello[split_at..]).await?;

    let payload = server.await??;
    assert_eq!(payload, hello);
    assert_eq!(
        extract_server_name(&payload).as_deref(),
        Some("abc.example.com")
    );
    Ok(())
}

#[tokio::test]
async fn errors_when_connection_closes_before_preface() -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;

    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await?;
        read_preface(&mut stream).await
    });

    let client = TcpStream::connect(addr).await?;
    drop(client);

    let err = server
        .await
        .expect("server task should join")
        .expect_err("read_preface should fail on empty connection");
    assert_eq!(
        err.to_string(),
        "stream dispatch error: connection closed before client preface"
    );
    Ok(())
}

#[tokio::test]
async fn errors_when_preface_read_times_out() -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;

    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await?;
        read_preface(&mut stream).await
    });

    let _client = TcpStream::connect(addr).await?;

    let err = timeout(Duration::from_millis(1_500), server)
        .await
        .expect("preface read should respect its configured timeout")
        .expect("server task should join")
        .expect_err("read_preface should fail when no preface bytes arrive");
    assert_eq!(
        err.to_string(),
        "stream dispatch error: timed out reading client preface"
    );
    Ok(())
}
