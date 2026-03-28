//! Batch tool — run multiple tool operations in a single MCP call.

use crate::dispatch::DispatchFn;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Semaphore;

/// A single operation in a batch request.
#[derive(Debug, Deserialize)]
pub struct BatchOperation {
    pub tool: String,
    pub params: Value,
}

/// Result of a single operation in the batch.
#[derive(Debug, Serialize)]
pub struct BatchOperationResult {
    pub tool: String,
    pub params: Value,
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub duration_ms: u64,
}

/// Full batch result.
#[derive(Debug, Serialize)]
pub struct BatchResult {
    pub results: Vec<BatchOperationResult>,
    pub total_duration_ms: u64,
}

/// Execute a batch of operations in parallel.
pub async fn execute_batch(
    operations: Vec<BatchOperation>,
    dispatch: &HashMap<String, DispatchFn>,
    concurrency: usize,
    per_op_timeout_secs: u64,
) -> BatchResult {
    let start = Instant::now();
    let semaphore = Arc::new(Semaphore::new(concurrency));
    let dispatch = Arc::new(dispatch.clone());

    let mut handles = Vec::with_capacity(operations.len());

    for (idx, op) in operations.into_iter().enumerate() {
        let sem = semaphore.clone();
        let dispatch = dispatch.clone();
        let timeout = std::time::Duration::from_secs(per_op_timeout_secs);

        handles.push(tokio::spawn(async move {
            let _permit = sem.acquire().await.unwrap();
            let op_start = Instant::now();

            let result = if let Some(func) = dispatch.get(&op.tool) {
                match tokio::time::timeout(timeout, func(op.params.clone())).await {
                    Ok(Ok(val)) => BatchOperationResult {
                        tool: op.tool,
                        params: op.params,
                        success: true,
                        result: Some(val),
                        error: None,
                        duration_ms: op_start.elapsed().as_millis() as u64,
                    },
                    Ok(Err(e)) => BatchOperationResult {
                        tool: op.tool,
                        params: op.params,
                        success: false,
                        result: None,
                        error: Some(e),
                        duration_ms: op_start.elapsed().as_millis() as u64,
                    },
                    Err(_) => BatchOperationResult {
                        tool: op.tool,
                        params: op.params,
                        success: false,
                        result: None,
                        error: Some(format!("operation timed out after {per_op_timeout_secs}s")),
                        duration_ms: op_start.elapsed().as_millis() as u64,
                    },
                }
            } else {
                let registered: Vec<&String> = dispatch.keys().collect();
                BatchOperationResult {
                    tool: op.tool.clone(),
                    params: op.params,
                    success: false,
                    result: None,
                    error: Some(format!(
                        "tool '{}' is not registered. Registered tools: {}",
                        op.tool,
                        registered.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")
                    )),
                    duration_ms: 0,
                }
            };

            (idx, result)
        }));
    }

    let mut results: Vec<(usize, BatchOperationResult)> = Vec::new();
    for handle in handles {
        if let Ok(result) = handle.await {
            results.push(result);
        }
    }

    // Sort by original index to preserve order
    results.sort_by_key(|(idx, _)| *idx);

    BatchResult {
        results: results.into_iter().map(|(_, r)| r).collect(),
        total_duration_ms: start.elapsed().as_millis() as u64,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dispatch::build_dispatch_table;

    #[tokio::test]
    async fn batch_ls_and_wc() {
        let table = build_dispatch_table(&None);
        let ops = vec![
            BatchOperation {
                tool: "wc".to_string(),
                params: serde_json::json!({"input": "hello world"}),
            },
            BatchOperation {
                tool: "wc".to_string(),
                params: serde_json::json!({"input": "foo bar baz"}),
            },
        ];

        let result = execute_batch(ops, &table, 4, 30).await;
        assert_eq!(result.results.len(), 2);
        assert!(result.results[0].success);
        assert!(result.results[1].success);
    }

    #[tokio::test]
    async fn batch_unregistered_tool() {
        let mut enabled = std::collections::HashSet::new();
        enabled.insert("wc".to_string());
        let table = build_dispatch_table(&Some(enabled));

        let ops = vec![
            BatchOperation {
                tool: "wc".to_string(),
                params: serde_json::json!({"input": "test"}),
            },
            BatchOperation {
                tool: "ls".to_string(),
                params: serde_json::json!({"path": "."}),
            },
        ];

        let result = execute_batch(ops, &table, 4, 30).await;
        assert!(result.results[0].success);
        assert!(!result.results[1].success);
        assert!(result.results[1].error.as_ref().unwrap().contains("not registered"));
    }

    #[tokio::test]
    async fn batch_preserves_order() {
        let table = build_dispatch_table(&None);
        let ops = vec![
            BatchOperation {
                tool: "wc".to_string(),
                params: serde_json::json!({"input": "first"}),
            },
            BatchOperation {
                tool: "wc".to_string(),
                params: serde_json::json!({"input": "second"}),
            },
            BatchOperation {
                tool: "wc".to_string(),
                params: serde_json::json!({"input": "third"}),
            },
        ];

        let result = execute_batch(ops, &table, 4, 30).await;
        assert_eq!(result.results.len(), 3);
        // Params echoed for correlation
        assert_eq!(result.results[0].params["input"], "first");
        assert_eq!(result.results[2].params["input"], "third");
    }
}
