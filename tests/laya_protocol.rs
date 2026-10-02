use context_foundry::laya::{self, Strategy};
use std::{
    io::{Read, Write},
    net::TcpListener,
    thread,
    time::{Duration, Instant},
};

fn server(body: &'static str, status: &'static str) -> (u16, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    listener.set_nonblocking(true).unwrap();
    let worker = thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut socket = loop {
            match listener.accept() {
                Ok((socket, _)) => break socket,
                Err(e)
                    if e.kind() == std::io::ErrorKind::WouldBlock && Instant::now() < deadline =>
                {
                    thread::sleep(Duration::from_millis(5))
                }
                Err(e) => panic!("fixture accept failed: {e}"),
            }
        };
        // The listener is non-blocking for the accept poll; the accepted
        // stream inherits that mode and would fail reads with WouldBlock.
        socket.set_nonblocking(false).unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut request = [0u8; 16384];
        let n = socket.read(&mut request).unwrap();
        assert!(
            std::str::from_utf8(&request[..n])
                .unwrap()
                .starts_with("POST /v1/systemone HTTP/1.1")
        );
        write!(socket, "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
    });
    (port, worker)
}

#[test]
fn real_http_transport_reads_laya_choice_and_answer_confidence() {
    let (port, worker) = server(
        r#"{"answers":{"strategy":{"type":"choice","choice":"graph","confidence":0.1,"answer_confidence":0.93}}}"#,
        "200 OK",
    );
    let decision = laya::decide("find parser", Some(port), 0.8);
    worker.join().unwrap();
    assert_eq!(decision.source, "laya");
    assert_eq!(decision.strategy, Strategy::Graph);
    assert_eq!(decision.answer_confidence, Some(0.93));
}

#[test]
fn malformed_model_output_preserves_deterministic_retrieval() {
    let (port, worker) = server(
        r#"{"answers":{"strategy":{"type":"choice","choice":"delete_workspace","answer_confidence":0.99}}}"#,
        "200 OK",
    );
    let decision = laya::decide("find parser", Some(port), 0.8);
    worker.join().unwrap();
    assert_eq!(decision.strategy, Strategy::Search);
    assert_eq!(decision.source, "deterministic");
    assert!(
        decision
            .fallback_reason
            .unwrap()
            .contains("unknown strategy")
    );
}

#[test]
fn missing_calibrated_confidence_is_not_invented() {
    let (port, worker) = server(
        r#"{"answers":{"strategy":{"type":"choice","choice":"search","confidence":0.99}}}"#,
        "200 OK",
    );
    let decision = laya::decide("who calls parser", Some(port), 0.8);
    worker.join().unwrap();
    assert_eq!(decision.strategy, Strategy::Graph);
    assert_eq!(decision.source, "deterministic");
    assert!(
        decision
            .fallback_reason
            .unwrap()
            .contains("answer_confidence")
    );
}
