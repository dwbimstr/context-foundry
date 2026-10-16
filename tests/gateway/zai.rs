//! Real gateway and launcher boundaries with local proxy/provider fixtures.
use super::*;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn standard_api_uses_the_configured_proxy_and_keeps_unreported_cost_unknown() {
    let upstream = FakeUpstream::start().await;
    let proxy = FakeUpstream::start().await;
    proxy.queue(Reply::sse("", simple_stream()));
    let dir = tempfile::tempdir().unwrap();
    let mut gateway = Gateway::start(
        dir.path(),
        Spawn {
            upstream: Some(upstream.port),
            standard_api: true,
            environment: vec![("HTTP_PROXY", format!("http://127.0.0.1:{}", proxy.port))],
            log_bytes: Some(65_536),
            ..Default::default()
        },
    );
    assert_eq!(gateway.ready["billing_mode"], "standard_api");
    assert_eq!(
        gateway.ready["upstream"],
        context_foundry::gateway::STANDARD_UPSTREAM
    );
    let response = gateway.post(fixture(TURN)).await;
    assert_eq!(response.status(), 200);
    assert!(read_all(response).await.1);
    let receipts = gateway.wait_receipts(1).await;
    assert_eq!(receipts[0].outcome, Outcome::Complete);
    assert!(receipts[0].cost_microunits.is_none());
    assert_eq!(upstream.count(), 0, "no direct request bypasses the proxy");
    assert_eq!(proxy.count(), 1);
    assert!(proxy.received()[0].head.starts_with(&format!(
        "POST http://127.0.0.1:{}/api/paas/v4/chat/completions HTTP/1.1",
        upstream.port
    )));
    gateway.terminate().assert_no_secrets();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn https_proxy_failure_is_recorded_without_direct_fallback() {
    let proxy = FakeUpstream::start().await;
    proxy.queue(Reply::status(502, "Bad Gateway", "", KEY_CANARY));
    let dir = tempfile::tempdir().unwrap();
    let mut gateway = Gateway::start(
        dir.path(),
        Spawn {
            environment: vec![("HTTPS_PROXY", format!("http://127.0.0.1:{}", proxy.port))],
            log_bytes: Some(65_536),
            idle_ms: Some(1500),
            deadline_ms: Some(3000),
            ..Default::default()
        },
    );
    assert_eq!(gateway.ready["billing_mode"], "coding_plan");
    let response = gateway.post(fixture(TURN)).await;
    assert_eq!(response.status(), 502);
    let body = response.text().await.unwrap();
    assert!(!body.contains(KEY_CANARY));
    let receipts = gateway.wait_receipts(1).await;
    assert_ne!(receipts[0].outcome, Outcome::Complete);
    assert!(receipts[0].input_tokens.is_none());
    assert_eq!(proxy.count(), 1);
    let received = proxy.received();
    assert!(
        received[0]
            .head
            .starts_with("CONNECT api.z.ai:443 HTTP/1.1")
    );
    assert!(received[0].header("authorization").is_none());
    gateway.terminate().assert_no_secrets();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn explicit_no_proxy_keeps_loopback_traffic_local() {
    let upstream = FakeUpstream::start().await;
    let proxy = FakeUpstream::start().await;
    upstream.queue(Reply::sse("", simple_stream()));
    let dir = tempfile::tempdir().unwrap();
    let mut gateway = Gateway::start(
        dir.path(),
        Spawn {
            upstream: Some(upstream.port),
            environment: vec![
                ("HTTP_PROXY", format!("http://127.0.0.1:{}", proxy.port)),
                ("NO_PROXY", "127.0.0.1".to_owned()),
            ],
            log_bytes: Some(65_536),
            ..Default::default()
        },
    );
    let response = gateway.post(fixture(TURN)).await;
    assert_eq!(response.status(), 200);
    assert!(read_all(response).await.1);
    gateway.wait_receipts(1).await;
    assert_eq!(upstream.count(), 1);
    assert_eq!(proxy.count(), 0);
    gateway.terminate().assert_no_secrets();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn launcher_accepts_an_environment_binding_and_keeps_transport_out_of_omp() {
    let proxy = FakeUpstream::start().await;
    proxy.queue(Reply::sse("", simple_stream()));
    let upstream = FakeUpstream::start().await;
    let launcher = Launcher::new();
    let config = launcher.config("env-run");
    let out = launcher.work.path().join("omp-out");
    let omp = launcher.fake_omp(&out, "env-run", "env-profile", 0);
    let script = std::fs::read_to_string(&omp).unwrap().replace(
        "exit 0",
        &format!(
            "exec '{}' --exact zai::launcher_http_fixture --ignored --nocapture",
            std::env::current_exe().unwrap().display()
        ),
    );
    std::fs::write(&omp, script).unwrap();
    let mut command = Command::new(BIN);
    command
        .args([
            "gateway-omp",
            "--config",
            path_str(&config),
            "--profile",
            "env-profile",
            "--omp",
            path_str(&omp),
        ])
        .env_clear()
        .env("HOME", launcher.home.path())
        .env("PATH", "/usr/bin:/bin")
        .env(KEY_ENV, KEY_CANARY)
        .env("HTTP_PROXY", format!("http://127.0.0.1:{}", proxy.port))
        .env(
            "FOUNDRY_GATEWAY_TEST_UPSTREAM",
            format!("http://127.0.0.1:{}/api/coding/paas/v4", upstream.port),
        )
        .stdin(Stdio::null());
    // Use the host's actual CA bundle when explicitly configured. A nonexistent
    // sentinel would correctly make TLS client initialization fail on Linux.
    if let Some(bundle) = std::env::var_os("SSL_CERT_FILE") {
        command.env("SSL_CERT_FILE", bundle);
    }
    let output =
        tokio::task::spawn_blocking(move || run_with_deadline(command, Duration::from_secs(30)))
            .await
            .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(proxy.count(), 1, "the gateway child inherited the proxy");
    assert_eq!(upstream.count(), 0);
    assert_eq!(
        proxy.received()[0].header("authorization").as_deref(),
        Some(format!("Bearer {KEY_CANARY}").as_str())
    );
    let host = std::fs::read_to_string(out.join("env")).unwrap();
    assert!(!host.contains(KEY_CANARY));
    for name in [KEY_ENV, "HTTP_PROXY", "SSL_CERT_FILE"] {
        assert!(!env_map(&host).contains_key(name), "{name} reached OMP");
    }
    assert!(!String::from_utf8_lossy(&output.stderr).contains(KEY_CANARY));
    assert!(!launcher.run_dir("env-run").join("token").exists());
    launcher.assert_global_state_untouched();
}

#[test]
#[ignore = "subprocess fixture invoked by launcher_accepts_an_environment_binding"]
fn launcher_http_fixture() {
    use std::io::Write as _;
    let home = std::env::var_os("HOME").unwrap();
    let models = std::fs::read_to_string(
        Path::new(&home).join(".omp/profiles/env-profile/agent/models.yml"),
    )
    .unwrap();
    let port: u16 = models
        .split("127.0.0.1:")
        .nth(1)
        .unwrap()
        .split('/')
        .next()
        .unwrap()
        .parse()
        .unwrap();
    let token = std::env::var("FOUNDRY_GATEWAY_TOKEN").unwrap();
    let mut socket = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
    socket
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let body = fixture(TURN);
    write!(socket, "POST /v1/chat/completions HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nAuthorization: Bearer {token}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len()).unwrap();
    socket.write_all(&body).unwrap();
    let mut response = String::new();
    socket.take(65_536).read_to_string(&mut response).unwrap();
    assert!(response.starts_with("HTTP/1.1 200"));
    assert!(response.contains("[DONE]"));
}

#[test]
fn missing_environment_credential_refuses_before_creating_gateway_or_profile() {
    let launcher = Launcher::new();
    let config = launcher.config("missing-key");
    let output = launcher.launch(
        BIN,
        &["--config", path_str(&config), "--profile", "missing"],
        None,
    );
    assert!(!output.status.success());
    assert_eq!(error_code(&output), "gateway_credential_missing");
    assert!(!launcher.run_dir("missing-key").exists());
    assert!(!launcher.profile_dir("missing").exists());
}
