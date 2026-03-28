//! Free functions for tool dispatch. Called by both the MCP tool_router
//! and the batch/pipe meta-tools.

use serde_json::Value;

use crate::{diff, docker, kubectl, ls, lsof, sqlite, wc};

/// Dispatch result: either a JSON value or an error string.
pub type DispatchResult = Result<Value, String>;

// ── ls ────────────────────────────────────────────────────────────────

pub async fn do_ls(params: Value) -> DispatchResult {
    let path = params.get("path").and_then(|v| v.as_str()).unwrap_or(".");
    let all = params.get("all").and_then(|v| v.as_bool()).unwrap_or(false);
    let long = params.get("long").and_then(|v| v.as_bool()).unwrap_or(true);

    let entries = ls::list_directory(path, all, long)
        .await
        .map_err(|e| e.to_string())?;
    serde_json::to_value(&entries).map_err(|e| e.to_string())
}

// ── wc ────────────────────────────────────────────────────────────────

pub async fn do_wc(params: Value) -> DispatchResult {
    // Support both single path/input and paths array
    if let Some(paths) = params.get("paths").and_then(|v| v.as_array()) {
        let mut results = Vec::new();
        for p in paths {
            if let Some(path_str) = p.as_str() {
                match wc::word_count(path_str).await {
                    Ok(r) => results.push(serde_json::to_value(&r).map_err(|e| e.to_string())?),
                    Err(e) => results.push(serde_json::json!({
                        "file": path_str,
                        "error": e.to_string()
                    })),
                }
            }
        }
        if results.len() == 1 {
            return Ok(results.into_iter().next().unwrap());
        }
        return Ok(Value::Array(results));
    }

    let path = params.get("path").and_then(|v| v.as_str());
    let input = params.get("input").and_then(|v| v.as_str());

    match (path, input) {
        (Some(p), None) => {
            let r = wc::word_count(p).await.map_err(|e| e.to_string())?;
            serde_json::to_value(&r).map_err(|e| e.to_string())
        }
        (None, Some(i)) => {
            let r = wc::word_count_str(i);
            serde_json::to_value(&r).map_err(|e| e.to_string())
        }
        (Some(_), Some(_)) => Err("Provide either 'path', 'input', or 'paths', not multiple.".into()),
        (None, None) => Err("Provide 'path', 'input', or 'paths'.".into()),
    }
}

// ── diff ──────────────────────────────────────────────────────────────

pub async fn do_diff(params: Value) -> DispatchResult {
    let input = params.get("input").and_then(|v| v.as_str());
    let git_args = params.get("git_args").and_then(|v| v.as_array());

    let diff_text = match (input, git_args) {
        (Some(text), None) => text.to_string(),
        (None, Some(args)) => {
            let arg_strs: Vec<String> = args.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
            let arg_refs: Vec<&str> = arg_strs.iter().map(|s| s.as_str()).collect();
            diff::run_diff(&arg_refs).await.map_err(|e| format!("git diff failed: {e}"))?
        }
        (Some(_), Some(_)) => return Err("Provide either 'input' or 'git_args', not both.".into()),
        (None, None) => return Err("Provide either 'input' or 'git_args'.".into()),
    };

    if diff_text.trim().is_empty() {
        return Ok(serde_json::json!({
            "format": "unified",
            "files": [],
            "total_additions": 0,
            "total_deletions": 0
        }));
    }

    match diff::parse_unified_diff(&diff_text) {
        Ok(result) => serde_json::to_value(&result).map_err(|e| e.to_string()),
        Err(fmt_err) => {
            let msg = serde_json::to_string(&fmt_err).unwrap_or(fmt_err.error);
            Err(msg)
        }
    }
}

// ── lsof ──────────────────────────────────────────────────────────────

pub async fn do_lsof(params: Value) -> DispatchResult {
    let mut args: Vec<String> = vec!["-n".to_string(), "-P".to_string()];

    if let Some(port) = params.get("port").and_then(|v| v.as_str()) {
        if port.starts_with(':') {
            args.push(format!("-i{port}"));
        } else {
            args.push(format!("-i:{port}"));
        }
    }

    if let Some(pid) = params.get("pid").and_then(|v| v.as_str()) {
        args.push("-p".to_string());
        args.push(pid.to_string());
    }

    if let Some(proto) = params.get("protocol").and_then(|v| v.as_str()) {
        args.push(format!("-i{proto}"));
    }

    if params.get("network_only").and_then(|v| v.as_bool()).unwrap_or(false)
        && params.get("port").is_none()
        && params.get("protocol").is_none()
    {
        args.push("-i".to_string());
    }

    if let Some(extra) = params.get("extra_args").and_then(|v| v.as_array()) {
        for a in extra {
            if let Some(s) = a.as_str() {
                args.push(s.to_string());
            }
        }
    }

    let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let output = lsof::run_lsof(&arg_refs).await.map_err(|e| format!("lsof failed: {e}"))?;
    let result = lsof::parse_lsof_output(&output);
    serde_json::to_value(&result).map_err(|e| e.to_string())
}

// ── kubectl_list ──────────────────────────────────────────────────────

pub async fn do_kubectl_list(params: Value) -> DispatchResult {
    let resource_type = params.get("resource_type")
        .and_then(|v| v.as_str())
        .ok_or("missing required field 'resource_type'")?;
    let ns = params.get("namespace").and_then(|v| v.as_str()).unwrap_or("default");

    let mut extra: Vec<String> = Vec::new();
    if let Some(sel) = params.get("label_selector").and_then(|v| v.as_str()) {
        extra.push("-l".to_string());
        extra.push(sel.to_string());
    }
    let extra_refs: Vec<&str> = extra.iter().map(|s| s.as_str()).collect();

    let output = kubectl::kubectl_get(resource_type, None, ns, &extra_refs)
        .await
        .map_err(|e| format!("kubectl failed: {e}"))?;
    let result = kubectl::parse_list_response(&output, resource_type, ns)
        .map_err(|e| e.to_string())?;
    serde_json::to_value(&result).map_err(|e| e.to_string())
}

// ── kubectl_get ───────────────────────────────────────────────────────

pub async fn do_kubectl_get(params: Value) -> DispatchResult {
    let resource_type = params.get("resource_type")
        .and_then(|v| v.as_str())
        .ok_or("missing required field 'resource_type'")?;
    let name = params.get("name")
        .and_then(|v| v.as_str())
        .ok_or("missing required field 'name'")?;
    let ns = params.get("namespace").and_then(|v| v.as_str()).unwrap_or("default");

    let output = kubectl::kubectl_get(resource_type, Some(name), ns, &[])
        .await
        .map_err(|e| format!("kubectl failed: {e}"))?;
    let result = kubectl::parse_get_response(&output).map_err(|e| e.to_string())?;
    serde_json::to_value(&result).map_err(|e| e.to_string())
}

// ── docker_list ───────────────────────────────────────────────────────

pub async fn do_docker_list(params: Value) -> DispatchResult {
    let all = params.get("all").and_then(|v| v.as_bool()).unwrap_or(false);
    let client = docker::connect().map_err(|e| e.to_string())?;
    let result = docker::list_containers(&client, all).await.map_err(|e| e.to_string())?;
    serde_json::to_value(&result).map_err(|e| e.to_string())
}

// ── docker_inspect ────────────────────────────────────────────────────

pub async fn do_docker_inspect(params: Value) -> DispatchResult {
    let container = params.get("container")
        .and_then(|v| v.as_str())
        .ok_or("missing required field 'container'")?;
    let client = docker::connect().map_err(|e| e.to_string())?;
    let result = docker::inspect_container(&client, container).await.map_err(|e| e.to_string())?;
    serde_json::to_value(&result).map_err(|e| e.to_string())
}

// ── docker_images ─────────────────────────────────────────────────────

pub async fn do_docker_images(_params: Value) -> DispatchResult {
    let client = docker::connect().map_err(|e| e.to_string())?;
    let result = docker::list_images(&client).await.map_err(|e| e.to_string())?;
    serde_json::to_value(&result).map_err(|e| e.to_string())
}

// ── sqlite_query ──────────────────────────────────────────────────────

pub async fn do_sqlite_query(params: Value) -> DispatchResult {
    let db_path = params.get("db_path")
        .and_then(|v| v.as_str())
        .ok_or("missing required field 'db_path'")?;
    let sql = params.get("sql")
        .and_then(|v| v.as_str())
        .ok_or("missing required field 'sql'")?;
    let result = sqlite::query(db_path, sql)?;
    serde_json::to_value(&result).map_err(|e| e.to_string())
}

// ── sqlite_tables ─────────────────────────────────────────────────────

pub async fn do_sqlite_tables(params: Value) -> DispatchResult {
    let db_path = params.get("db_path")
        .and_then(|v| v.as_str())
        .ok_or("missing required field 'db_path'")?;
    let tables = sqlite::list_tables(db_path)?;
    serde_json::to_value(&tables).map_err(|e| e.to_string())
}

// ── Dispatch table ────────────────────────────────────────────────────

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

pub type DispatchFn = Arc<dyn Fn(Value) -> Pin<Box<dyn Future<Output = DispatchResult> + Send>> + Send + Sync>;

/// Build the dispatch table for all enabled tools.
pub fn build_dispatch_table(enabled: &Option<std::collections::HashSet<String>>) -> HashMap<String, DispatchFn> {
    let mut table: HashMap<String, DispatchFn> = HashMap::new();

    let is_enabled = |name: &str| -> bool {
        enabled.as_ref().is_none_or(|set| set.contains(name))
    };

    macro_rules! register {
        ($name:expr, $func:ident) => {
            if is_enabled($name) {
                table.insert($name.to_string(), Arc::new(|params| Box::pin($func(params))));
            }
        };
    }

    register!("ls", do_ls);
    register!("wc", do_wc);
    register!("diff", do_diff);
    register!("lsof", do_lsof);
    register!("kubectl_list", do_kubectl_list);
    register!("kubectl_get", do_kubectl_get);
    register!("docker_list", do_docker_list);
    register!("docker_inspect", do_docker_inspect);
    register!("docker_images", do_docker_images);
    register!("sqlite_query", do_sqlite_query);
    register!("sqlite_tables", do_sqlite_tables);

    table
}
