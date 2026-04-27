//! E2E test entry point. `cargo test --test e2e` runs everything.
//!
//! Real-infra tests (docker/k8s/gh) are gated by the `integration-real` feature.

mod harness;

mod batch_tests;
mod curl_tests;
mod diff_tests;
mod find_tests;
mod git_tests;
mod ls_tests;
mod mcp_protocol_tests;
mod pipe_tests;
mod property_tests;
mod real_infra_tests;
mod snapshot_tests;
mod sqlite_tests;
mod system_tests;
mod tools_flag_tests;
mod wc_tests;
