//! Contract cases for the IPC part of `doc/spec/ewmh.md` that need no Ruby: the socket
//! and serving clients. Requests that run Ruby are in `config.rs`.

mod common;

use std::io::Read;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::time::{Duration, Instant};

use common::*;
use sugoi::Settings;
use sugoictl::{Reply, request, socket_path};

/// [I1]
#[test]
fn i1_the_socket_is_the_users_only() {
    let session = Session::with_settings(Settings::default());
    let Some(path) = socket_path(&session.x.display) else { return };
    eventually("socket", || path.exists());
    let mode = |p: &std::path::Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(&path), 0o600);
    assert_eq!(mode(path.parent().unwrap()), 0o700);
}

/// [I2] Without Ruby, a request is answered with an error.
#[test]
fn i2_a_request_without_ruby_is_an_error() {
    let session = Session::with_settings(Settings::default());
    let Some(path) = socket_path(&session.x.display) else { return };
    eventually("socket", || path.exists());
    let reply = request(&path, "quit").unwrap();
    assert_eq!(reply, Reply::Error("this sugoi runs without Ruby".into()));
}

/// [I4]
#[test]
fn i4_a_silent_client_neither_blocks_others_nor_stays() {
    let session = Session::with_settings(Settings::default());
    let Some(path) = socket_path(&session.x.display) else { return };
    eventually("socket", || path.exists());
    let mut silent = UnixStream::connect(&path).unwrap();
    let started = Instant::now();
    assert!(matches!(request(&path, "quit"), Ok(Reply::Error(_))));
    assert!(started.elapsed() < Duration::from_millis(500));
    // Dropped after its second: the connection reads as closed.
    silent.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
    let mut rest = Vec::new();
    assert_eq!(silent.read_to_end(&mut rest).unwrap(), 0);
    assert!(started.elapsed() >= Duration::from_millis(900));
}
