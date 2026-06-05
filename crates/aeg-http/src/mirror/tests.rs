use bytes::Bytes;
use pingora::protocols::http::HttpTask;
use tokio::{sync::mpsc, sync::mpsc::error::TryRecvError, time::Duration};

use super::{
    configure_request_mirror_budget, forward_body_chunk, request_mirror_semaphore,
    wait_for_request_mirrors, MirrorRequestSession,
};

#[tokio::test]
async fn forward_body_chunk_sends_body_and_done() {
    let (tx, mut rx) = mpsc::channel(4);
    let mirror = MirrorRequestSession {
        tx,
        forwards_body: true,
        backend_name: "shadow/default".to_string(),
        completion: None,
    };

    assert!(forward_body_chunk(&mirror, &Some(Bytes::from_static(b"hello")), true).await);

    match rx.recv().await.expect("body task") {
        HttpTask::Body(Some(body), false) => assert_eq!(body, Bytes::from_static(b"hello")),
        other => panic!("unexpected task: {other:?}"),
    }
    assert!(matches!(
        rx.recv().await.expect("done task"),
        HttpTask::Done
    ));
}

#[tokio::test]
async fn forward_body_chunk_drops_overloaded_mirror_without_blocking() {
    let (tx, _rx) = mpsc::channel(1);
    let mirror = MirrorRequestSession {
        tx,
        forwards_body: true,
        backend_name: "shadow/default".to_string(),
        completion: None,
    };

    assert!(forward_body_chunk(&mirror, &Some(Bytes::from_static(b"hello")), false).await);
    assert!(!forward_body_chunk(&mirror, &Some(Bytes::from_static(b"world")), false).await);
}

#[tokio::test]
async fn forward_body_chunk_keeps_bodyless_mirror_alive_for_completion_wait() {
    let (tx, mut rx) = mpsc::channel(1);
    let mirror = MirrorRequestSession {
        tx,
        forwards_body: false,
        backend_name: "shadow/default".to_string(),
        completion: None,
    };

    assert!(forward_body_chunk(&mirror, &Some(Bytes::from_static(b"hello")), true).await);
    assert!(matches!(rx.try_recv(), Err(TryRecvError::Empty)));
}

#[tokio::test]
async fn wait_for_request_mirrors_drains_bodyless_completion_handles() {
    let completion = tokio::spawn(async {
        tokio::time::sleep(Duration::from_millis(5)).await;
    });
    let (tx, _rx) = mpsc::channel(1);
    let mut mirrors = vec![MirrorRequestSession {
        tx,
        forwards_body: false,
        backend_name: "shadow/default".to_string(),
        completion: Some(completion),
    }];

    wait_for_request_mirrors(&mut mirrors).await;

    assert!(mirrors.is_empty());
}

#[test]
fn request_mirror_budget_can_be_configured_once() {
    configure_request_mirror_budget(7);

    assert_eq!(request_mirror_semaphore().available_permits(), 7);
}
