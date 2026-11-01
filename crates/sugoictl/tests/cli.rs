//! [I3] sugoictl's exit codes and output, against a fake sugoi.

use std::io::{Read, Write};
use std::os::unix::net::UnixListener;
use std::process::Command;
use std::thread;

/// Runs sugoictl against a server that answers every request with `reply`.
fn run(reply: Option<&'static str>, args: &[&str]) -> std::process::Output {
    let dir = std::env::temp_dir().join(format!(
        "sugoictl-test-{}-{}",
        std::process::id(),
        args.join("_").len()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("{}.sock", reply.map_or(0, str::len)));
    let _ = std::fs::remove_file(&path);
    if let Some(reply) = reply {
        let listener = UnixListener::bind(&path).unwrap();
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = String::new();
            stream.read_to_string(&mut request).unwrap();
            stream.write_all(reply.as_bytes()).unwrap();
        });
    }
    let output = Command::new(env!("CARGO_BIN_EXE_sugoictl"))
        .args(args)
        .env("SUGOI_SOCKET", &path)
        .output()
        .unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    output
}

#[test]
fn a_value_is_printed_with_status_0() {
    let output = run(Some("ok\n[1, 2]"), &["windows"]);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&output.stdout), "[1, 2]\n");
}

#[test]
fn an_error_is_printed_with_status_1() {
    let output = run(Some("error\nboom"), &["raise"]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(String::from_utf8_lossy(&output.stderr), "boom\n");
}

#[test]
fn no_sugoi_is_status_2() {
    let output = run(None, &["quit"]);
    assert_eq!(output.status.code(), Some(2));
}
