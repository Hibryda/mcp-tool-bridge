//! Tests against real external infrastructure: Docker daemon, kubectl, GitHub API, public httpbin.
//!
//! These are gated behind `--features integration-real` so they don't run in default CI.
//! Nightly CI enables the feature.

#![cfg(feature = "integration-real")]

use crate::harness::Server;
use serde_json::json;

// ── docker_list / docker_inspect / docker_images ────────────────────

#[test]
fn docker_list_returns_array() {
    let mut s = Server::spawn(&[]);
    let r = s.call("docker_list", json!({}));
    assert!(r.success());
    assert!(r.data["items"].is_array());
}

#[test]
fn docker_images_returns_array() {
    let mut s = Server::spawn(&[]);
    let r = s.call("docker_images", json!({}));
    assert!(r.success());
    assert!(r.data["items"].is_array());
}

#[test]
fn docker_inspect_unknown_container_errors() {
    let mut s = Server::spawn(&[]);
    let r = s.call(
        "docker_inspect",
        json!({"container": "definitely_not_a_container_xyz"}),
    );
    assert!(!r.success());
}

// ── kubectl ────────────────────────────────────────────────────────

#[test]
fn kubectl_list_namespaces() {
    let mut s = Server::spawn(&[]);
    let r = s.call("kubectl_list", json!({"resource_type": "namespaces"}));
    assert!(r.success());
    assert!(r.data["items"].is_array());
}

#[test]
fn kubectl_get_unknown_resource_errors() {
    let mut s = Server::spawn(&[]);
    let r = s.call(
        "kubectl_get",
        json!({
            "resource_type": "pod",
            "name": "definitely-not-a-pod-xyz",
            "namespace": "default"
        }),
    );
    assert!(!r.success());
}

// ── gh_api ─────────────────────────────────────────────────────────

#[test]
fn gh_api_rate_limit_endpoint() {
    let mut s = Server::spawn(&[]);
    let r = s.call("gh_api", json!({"endpoint": "/rate_limit"}));
    assert!(r.success());
    assert_eq!(r.data["status_code"], 200);
}

#[test]
fn gh_api_invalid_endpoint_rejected() {
    let mut s = Server::spawn(&[]);
    let r = s.call("gh_api", json!({"endpoint": "rate_limit"})); // missing leading /
    assert!(!r.success());
}

#[test]
fn gh_api_traversal_blocked() {
    let mut s = Server::spawn(&[]);
    let r = s.call("gh_api", json!({"endpoint": "/repos/../../etc/passwd"}));
    assert!(!r.success());
}

// ── curl against real httpbin (smoke test) ─────────────────────────

#[test]
fn curl_real_httpbin() {
    let mut s = Server::spawn(&[]);
    let r = s.call(
        "curl",
        json!({
            "url": "https://httpbin.org/get",
            "timeout_secs": 10
        }),
    );
    assert!(r.success());
    assert_eq!(r.data["status_code"], 200);
}
