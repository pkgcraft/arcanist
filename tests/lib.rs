use std::path::PathBuf;
use std::str;

use assert_cmd::Command as assert_command;
use once_cell::sync::Lazy;
use tempfile::Builder;

static TARGET_DIR: Lazy<String> = Lazy::new(|| {
    let tmp_dir = PathBuf::from(env!("CARGO_BIN_EXE_arcanist"));
    tmp_dir.parent().unwrap().to_str().unwrap().to_owned()
});

#[tokio::test]
async fn test_uds() {
    // ignore system/user config and run arcanist from build dir
    let env: [(&str, &str); 1] = [("PATH", &TARGET_DIR)];
    let args = ["--config", "no"];

    let tmp_dir = Builder::new().prefix("arcanist.").tempdir().unwrap();
    let socket_path = tmp_dir.path().to_owned().join("arcanist.sock");
    let socket = socket_path.to_str().unwrap();

    let (mut arcanist, socket) = arcanist::spawn(&socket, Some(env), Some(args), Some(5))
        .await
        .unwrap();

    let mut cmd = assert_command::cargo_bin("pakt").unwrap();
    let output = cmd
        .arg("--config")
        .arg("no")
        .arg("-c")
        .arg(&socket)
        .arg("version")
        .output()
        .unwrap();
    let expected = format!(
        "client: pakt-{0}, server: arcanist-{0}",
        env!("CARGO_PKG_VERSION")
    );
    assert_eq!(str::from_utf8(&output.stdout).unwrap().trim(), expected);

    arcanist.kill().await.unwrap();
}

#[tokio::test]
async fn test_tcp() {
    // ignore system/user config and run arcanist from build dir
    let env: [(&str, &str); 1] = [("PATH", &TARGET_DIR)];
    let args = ["--config", "no"];

    for addr in ["127.0.0.1:0", "[::]:0"] {
        let (mut arcanist, socket) = arcanist::spawn(addr, Some(env), Some(args), Some(5))
            .await
            .unwrap();
        let url = format!("http://{}", &socket);

        let expected = format!(
            "client: pakt-{0}, server: arcanist-{0}",
            env!("CARGO_PKG_VERSION")
        );

        // verify both raw socket and url args work
        for serve_addr in [socket, url] {
            let mut cmd = assert_command::cargo_bin("pakt").unwrap();
            let output = cmd
                .arg("--config")
                .arg("no")
                .arg("-c")
                .arg(&serve_addr)
                .arg("version")
                .output()
                .unwrap();
            assert_eq!(str::from_utf8(&output.stdout).unwrap().trim(), expected);
        }

        arcanist.kill().await.unwrap();
    }
}
