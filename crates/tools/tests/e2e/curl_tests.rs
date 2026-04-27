//! curl tests against a local mockito server (hermetic).

use crate::harness::Server;
use serde_json::json;

#[test]
fn curl_get_returns_status_and_body() {
    let mut srv = mockito::Server::new();
    let m = srv
        .mock("GET", "/test")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"ok": true, "n": 42}"#)
        .create();

    let mut s = Server::spawn(&[]);
    let r = s.call(
        "curl",
        json!({
            "url": format!("{}/test", srv.url()),
            "timeout_secs": 5
        }),
    );

    assert!(r.success());
    assert_eq!(r.data["status_code"], 200);
    assert_eq!(r.data["body_is_json"], true);
    m.assert();
}

#[test]
fn curl_404_status() {
    let mut srv = mockito::Server::new();
    let _m = srv.mock("GET", "/nope").with_status(404).create();
    let mut s = Server::spawn(&[]);
    let r = s.call(
        "curl",
        json!({
            "url": format!("{}/nope", srv.url()),
            "timeout_secs": 5
        }),
    );
    assert!(r.success());
    assert_eq!(r.data["status_code"], 404);
}

#[test]
fn curl_post_with_body() {
    let mut srv = mockito::Server::new();
    let _m = srv
        .mock("POST", "/echo")
        .match_body("data=value")
        .with_status(201)
        .with_body("created")
        .create();

    let mut s = Server::spawn(&[]);
    let r = s.call(
        "curl",
        json!({
            "url": format!("{}/echo", srv.url()),
            "method": "POST",
            "body": "data=value",
            "timeout_secs": 5
        }),
    );
    assert!(r.success());
    assert_eq!(r.data["status_code"], 201);
}

#[test]
fn curl_timing_present() {
    let mut srv = mockito::Server::new();
    let _m = srv
        .mock("GET", "/")
        .with_status(200)
        .with_body("ok")
        .create();
    let mut s = Server::spawn(&[]);
    let r = s.call("curl", json!({"url": srv.url(), "timeout_secs": 5}));
    assert!(r.success());
    assert!(r.data["timing"]["total_ms"].as_f64().unwrap() >= 0.0);
}

#[test]
fn curl_headers_returned() {
    let mut srv = mockito::Server::new();
    let _m = srv
        .mock("GET", "/h")
        .with_status(200)
        .with_header("x-custom", "abc")
        .with_body("body")
        .create();
    let mut s = Server::spawn(&[]);
    let r = s.call(
        "curl",
        json!({
            "url": format!("{}/h", srv.url()),
            "timeout_secs": 5
        }),
    );
    assert!(r.success());
    assert!(r.data["headers"].is_object());
}

#[test]
fn curl_non_json_body_detected() {
    let mut srv = mockito::Server::new();
    let _m = srv
        .mock("GET", "/text")
        .with_status(200)
        .with_body("plain text not json")
        .create();
    let mut s = Server::spawn(&[]);
    let r = s.call(
        "curl",
        json!({
            "url": format!("{}/text", srv.url()),
            "timeout_secs": 5
        }),
    );
    assert!(r.success());
    assert_eq!(r.data["body_is_json"], false);
}

#[test]
fn curl_connection_refused_errors() {
    let mut s = Server::spawn(&[]);
    // Port 1 is reserved/unbound on virtually all systems.
    let r = s.call(
        "curl",
        json!({
            "url": "http://127.0.0.1:1/no",
            "timeout_secs": 2
        }),
    );
    assert!(!r.success());
}

#[test]
fn curl_custom_request_headers_sent() {
    let mut srv = mockito::Server::new();
    let _m = srv
        .mock("GET", "/h")
        .match_header("x-test", "yes")
        .with_status(200)
        .create();
    let mut s = Server::spawn(&[]);
    let r = s.call(
        "curl",
        json!({
            "url": format!("{}/h", srv.url()),
            "headers": {"X-Test": "yes"},
            "timeout_secs": 5
        }),
    );
    assert!(r.success());
    assert_eq!(r.data["status_code"], 200);
}
