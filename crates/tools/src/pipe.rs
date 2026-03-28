//! Pipe tool — run a source tool and filter its array output on structured fields.

use crate::dispatch::DispatchFn;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// Allowed source tools that produce filterable array output.
const SOURCE_WHITELIST: &[&str] = &[
    "find", "ls", "lsof", "kubectl_list", "docker_list", "docker_images",
];

/// A single filter condition.
#[derive(Debug, Deserialize, Clone)]
pub struct Filter {
    /// JSON field name to match against.
    pub field: String,
    /// Pattern to match.
    pub pattern: String,
    /// Match mode.
    pub mode: FilterMode,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum FilterMode {
    Contains,
    Equals,
    StartsWith,
}

/// Pipe request.
#[derive(Debug, Deserialize)]
pub struct PipeRequest {
    pub source: PipeSource,
    pub filters: Vec<Filter>,
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct PipeSource {
    pub tool: String,
    pub params: Value,
}

/// Pipe result.
#[derive(Debug, Serialize)]
pub struct PipeResult {
    pub source_tool: String,
    pub total_before_filter: u64,
    pub total_after_filter: u64,
    pub items: Vec<Value>,
}

/// Execute a pipe: run source tool, filter results, apply limit.
pub async fn execute_pipe(
    request: PipeRequest,
    dispatch: &HashMap<String, DispatchFn>,
) -> Result<PipeResult, String> {
    // Validate source tool is whitelisted
    if !SOURCE_WHITELIST.contains(&request.source.tool.as_str()) {
        return Err(format!(
            "tool '{}' does not produce filterable array output. pipe supports: {}",
            request.source.tool,
            SOURCE_WHITELIST.join(", ")
        ));
    }

    // Get the dispatch function
    let func = dispatch.get(&request.source.tool)
        .ok_or_else(|| format!(
            "tool '{}' is not registered in this server instance",
            request.source.tool
        ))?;

    // Run source tool
    let source_result = func(request.source.params).await?;

    // Extract the array to filter
    let items = extract_array(&source_result, &request.source.tool)?;
    let total_before = items.len() as u64;

    // Check size guard (1MB)
    let source_size = serde_json::to_string(&source_result)
        .map(|s| s.len())
        .unwrap_or(0);
    if source_size > 1_048_576 {
        return Err(format!(
            "source output exceeds 1MB ({} bytes). Use more specific source params to reduce output.",
            source_size
        ));
    }

    // Apply filters (AND semantics)
    let filtered: Vec<Value> = items.into_iter()
        .filter(|item| {
            request.filters.iter().all(|f| matches_filter(item, f))
        })
        .collect();

    let total_after = filtered.len() as u64;

    // Apply limit
    let items = if let Some(limit) = request.limit {
        filtered.into_iter().take(limit).collect()
    } else {
        filtered
    };

    Ok(PipeResult {
        source_tool: request.source.tool,
        total_before_filter: total_before,
        total_after_filter: total_after,
        items,
    })
}

/// Extract the filterable array from a source tool's output.
/// Each tool has a different structure — we know which field contains the array.
fn extract_array(value: &Value, tool: &str) -> Result<Vec<Value>, String> {
    match tool {
        "ls" => {
            // ls returns a plain array
            value.as_array()
                .cloned()
                .ok_or_else(|| "ls output is not an array".to_string())
        }
        "find" => {
            // find returns {entries: [...]}
            value.get("entries")
                .and_then(|v| v.as_array())
                .cloned()
                .ok_or_else(|| "find output has no 'entries' array".to_string())
        }
        "lsof" => {
            // lsof returns {processes: [...]}
            value.get("processes")
                .and_then(|v| v.as_array())
                .cloned()
                .ok_or_else(|| "lsof output has no 'processes' array".to_string())
        }
        "kubectl_list" => {
            // kubectl_list returns {items: [...]}
            value.get("items")
                .and_then(|v| v.as_array())
                .cloned()
                .ok_or_else(|| "kubectl_list output has no 'items' array".to_string())
        }
        "docker_list" | "docker_images" => {
            // docker returns {items: [...]}
            value.get("items")
                .and_then(|v| v.as_array())
                .cloned()
                .ok_or_else(|| format!("{tool} output has no 'items' array"))
        }
        _ => Err(format!("no array extraction rule for tool '{tool}'")),
    }
}

/// Check if a JSON object matches a filter condition.
fn matches_filter(item: &Value, filter: &Filter) -> bool {
    // Support nested fields with dot notation (e.g., "metadata.name")
    let field_value = get_nested_field(item, &filter.field);

    let field_str = match field_value {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        Some(Value::Bool(b)) => b.to_string(),
        Some(Value::Null) => "null".to_string(),
        _ => return false,
    };

    match filter.mode {
        FilterMode::Contains => field_str.contains(&filter.pattern),
        FilterMode::Equals => field_str == filter.pattern,
        FilterMode::StartsWith => field_str.starts_with(&filter.pattern),
    }
}

/// Get a nested field value using dot notation.
fn get_nested_field<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    let mut current = value;
    for part in path.split('.') {
        current = current.get(part)?;
    }
    Some(current)
}

/// Build a field description for error messages — union of all fields across all items.
#[allow(dead_code)]
pub fn describe_available_fields(items: &[Value]) -> Vec<String> {
    let mut fields: HashMap<String, usize> = HashMap::new();
    for item in items {
        if let Some(obj) = item.as_object() {
            for key in obj.keys() {
                *fields.entry(key.clone()).or_insert(0) += 1;
            }
        }
    }
    let mut result: Vec<(String, usize)> = fields.into_iter().collect();
    result.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    result.into_iter().map(|(k, _)| k).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_contains() {
        let item = serde_json::json!({"name": "hello-world.rs", "type": "file"});
        let filter = Filter {
            field: "name".to_string(),
            pattern: "world".to_string(),
            mode: FilterMode::Contains,
        };
        assert!(matches_filter(&item, &filter));
    }

    #[test]
    fn filter_equals() {
        let item = serde_json::json!({"type": "file"});
        let f = Filter { field: "type".to_string(), pattern: "file".to_string(), mode: FilterMode::Equals };
        assert!(matches_filter(&item, &f));
        let f2 = Filter { field: "type".to_string(), pattern: "dir".to_string(), mode: FilterMode::Equals };
        assert!(!matches_filter(&item, &f2));
    }

    #[test]
    fn filter_starts_with() {
        let item = serde_json::json!({"status": "CrashLoopBackOff"});
        let f = Filter { field: "status".to_string(), pattern: "Crash".to_string(), mode: FilterMode::StartsWith };
        assert!(matches_filter(&item, &f));
    }

    #[test]
    fn filter_and_semantics() {
        let items = vec![
            serde_json::json!({"name": "a.rs", "type": "file"}),
            serde_json::json!({"name": "b.txt", "type": "file"}),
            serde_json::json!({"name": "c", "type": "directory"}),
        ];
        let filters = vec![
            Filter { field: "type".to_string(), pattern: "file".to_string(), mode: FilterMode::Equals },
            Filter { field: "name".to_string(), pattern: ".rs".to_string(), mode: FilterMode::Contains },
        ];
        let result: Vec<_> = items.into_iter()
            .filter(|item| filters.iter().all(|f| matches_filter(item, f)))
            .collect();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["name"], "a.rs");
    }

    #[test]
    fn nested_field_access() {
        let item = serde_json::json!({"metadata": {"name": "nginx", "namespace": "default"}});
        let f = Filter { field: "metadata.name".to_string(), pattern: "nginx".to_string(), mode: FilterMode::Equals };
        assert!(matches_filter(&item, &f));
    }

    #[test]
    fn whitelist_validation() {
        assert!(SOURCE_WHITELIST.contains(&"ls"));
        assert!(SOURCE_WHITELIST.contains(&"kubectl_list"));
        assert!(!SOURCE_WHITELIST.contains(&"diff"));
        assert!(!SOURCE_WHITELIST.contains(&"sqlite_query"));
    }

    #[test]
    fn describe_fields() {
        let items = vec![
            serde_json::json!({"name": "a", "size": 100}),
            serde_json::json!({"name": "b", "type": "file"}),
        ];
        let fields = describe_available_fields(&items);
        assert!(fields.contains(&"name".to_string()));
        assert!(fields.contains(&"size".to_string()));
        assert!(fields.contains(&"type".to_string()));
    }
}
