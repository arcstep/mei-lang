use std::net::TcpListener;
use std::process::Stdio;
use std::time::Duration;

#[tokio::test]
async fn serve_without_app_binds_control_plane_on_empty_workspace() {
    let workspace = tempfile::tempdir().expect("workspace");
    std::fs::write(
        workspace.path().join("workspace.json"),
        r#"{"schemaVersion":2,"workspace":{"id":"first-boot"}}"#,
    )
    .expect("workspace.json");
    let port = {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("reserve port");
        listener.local_addr().expect("local addr").port()
    };
    let port_string = port.to_string();
    let mut child = tokio::process::Command::new(env!("CARGO_BIN_EXE_mei-host-shell"))
        .args([
            "serve",
            "--workspace",
            workspace.path().to_str().expect("workspace path"),
            "--host",
            "127.0.0.1",
            "--port",
            port_string.as_str(),
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn host");
    let client = reqwest::Client::new();
    let base = format!("http://127.0.0.1:{port}");
    let mut bound = false;
    for _ in 0..50 {
        if client
            .get(format!("{base}/api/host/runtime/profile"))
            .send()
            .await
            .is_ok_and(|response| response.status().is_success())
        {
            bound = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(bound, "control plane did not bind");

    for path in [
        "/home",
        "/runtime",
        "/config",
        "/api/host/workspace-profiles",
    ] {
        let response = client
            .get(format!("{base}{path}"))
            .send()
            .await
            .expect("control route");
        assert!(
            response.status().is_success(),
            "{path}: {}",
            response.status()
        );
    }
    let profile: serde_json::Value = client
        .get(format!("{base}/api/host/runtime/profile"))
        .send()
        .await
        .expect("profile response")
        .json()
        .await
        .expect("profile json");
    assert_eq!(profile["status"], "unconfigured");
    assert_eq!(profile["access"]["state"], "unconfigured");
    assert_eq!(profile["selectedProfile"]["id"], "default");
    assert!(!workspace.path().join("apps").exists());
    assert!(!workspace
        .path()
        .join("deploy/state/host-control.json")
        .exists());

    child.kill().await.expect("stop host");
    let _ = child.wait().await;
}
