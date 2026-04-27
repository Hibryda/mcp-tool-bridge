use bridge_core::BridgeError;
use serde::Serialize;
use serde_json::Value;

/// Typed metadata fields stable since Kubernetes 1.0.
#[derive(Debug, Serialize, Clone)]
pub struct ResourceMetadata {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creation_timestamp: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub labels: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<Value>,
}

/// A single Kubernetes resource with typed metadata and passthrough spec/status.
#[derive(Debug, Serialize, Clone)]
pub struct KubeResource {
    pub api_version: String,
    pub kind: String,
    pub metadata: ResourceMetadata,
    /// Full spec — passthrough JSON, not typed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spec: Option<Value>,
    /// Full status — passthrough JSON, not typed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<Value>,
}

/// Result of a kubectl list query.
#[derive(Debug, Serialize, Clone)]
pub struct KubeListResult {
    pub kind: String,
    pub resource_type: String,
    pub namespace: String,
    pub items: Vec<KubeResource>,
    pub count: u64,
}

/// Result of a kubectl get (single resource).
#[derive(Debug, Serialize, Clone)]
pub struct KubeGetResult {
    pub resource: KubeResource,
}

/// Extract typed metadata from a raw K8s JSON object.
fn extract_metadata(obj: &Value) -> ResourceMetadata {
    let meta = obj.get("metadata").cloned().unwrap_or(Value::Null);
    ResourceMetadata {
        name: meta
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        namespace: meta
            .get("namespace")
            .and_then(|v| v.as_str())
            .map(String::from),
        uid: meta.get("uid").and_then(|v| v.as_str()).map(String::from),
        resource_version: meta
            .get("resourceVersion")
            .and_then(|v| v.as_str())
            .map(String::from),
        creation_timestamp: meta
            .get("creationTimestamp")
            .and_then(|v| v.as_str())
            .map(String::from),
        labels: meta.get("labels").cloned(),
        annotations: meta.get("annotations").cloned(),
    }
}

/// Parse a single K8s resource JSON object into typed struct.
fn parse_resource(obj: &Value) -> KubeResource {
    KubeResource {
        api_version: obj
            .get("apiVersion")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        kind: obj
            .get("kind")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        metadata: extract_metadata(obj),
        spec: obj.get("spec").cloned(),
        status: obj.get("status").cloned(),
    }
}

/// Parse kubectl -o json output for a list response.
pub fn parse_list_response(
    json_str: &str,
    resource_type: &str,
    namespace: &str,
) -> Result<KubeListResult, BridgeError> {
    let value: Value = serde_json::from_str(json_str)
        .map_err(|e| BridgeError::Parse(format!("invalid JSON from kubectl: {e}")))?;

    let kind = value
        .get("kind")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let items = value
        .get("items")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().map(parse_resource).collect::<Vec<_>>())
        .unwrap_or_default();

    let count = items.len() as u64;

    Ok(KubeListResult {
        kind,
        resource_type: resource_type.to_string(),
        namespace: namespace.to_string(),
        items,
        count,
    })
}

/// Parse kubectl -o json output for a single resource.
pub fn parse_get_response(json_str: &str) -> Result<KubeGetResult, BridgeError> {
    let value: Value = serde_json::from_str(json_str)
        .map_err(|e| BridgeError::Parse(format!("invalid JSON from kubectl: {e}")))?;

    Ok(KubeGetResult {
        resource: parse_resource(&value),
    })
}

/// Run kubectl get with -o json.
pub async fn kubectl_get(
    resource_type: &str,
    name: Option<&str>,
    namespace: &str,
    extra_args: &[&str],
) -> Result<String, BridgeError> {
    let mut args = vec!["get", resource_type];
    if let Some(n) = name {
        args.push(n);
    }
    args.extend_from_slice(&["-n", namespace, "-o", "json"]);
    args.extend_from_slice(extra_args);
    bridge_core::run_command("kubectl", &args).await
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_POD_LIST: &str = r#"{
        "apiVersion": "v1",
        "kind": "PodList",
        "items": [
            {
                "apiVersion": "v1",
                "kind": "Pod",
                "metadata": {
                    "name": "nginx-abc123",
                    "namespace": "default",
                    "uid": "12345-abcde",
                    "resourceVersion": "98765",
                    "creationTimestamp": "2026-03-28T10:00:00Z",
                    "labels": {"app": "nginx"}
                },
                "spec": {"containers": [{"name": "nginx", "image": "nginx:latest"}]},
                "status": {"phase": "Running"}
            },
            {
                "apiVersion": "v1",
                "kind": "Pod",
                "metadata": {
                    "name": "redis-def456",
                    "namespace": "default",
                    "uid": "67890-fghij",
                    "creationTimestamp": "2026-03-27T08:00:00Z",
                    "labels": {"app": "redis"}
                },
                "status": {"phase": "CrashLoopBackOff"}
            }
        ]
    }"#;

    #[test]
    fn parse_pod_list() {
        let result = parse_list_response(SAMPLE_POD_LIST, "pods", "default").unwrap();
        assert_eq!(result.kind, "PodList");
        assert_eq!(result.count, 2);
        assert_eq!(result.items[0].metadata.name, "nginx-abc123");
        assert_eq!(
            result.items[0].metadata.namespace,
            Some("default".to_string())
        );
        assert_eq!(result.items[0].kind, "Pod");
    }

    #[test]
    fn metadata_typed_correctly() {
        let result = parse_list_response(SAMPLE_POD_LIST, "pods", "default").unwrap();
        let pod = &result.items[0];
        assert_eq!(pod.metadata.uid, Some("12345-abcde".to_string()));
        assert_eq!(
            pod.metadata.creation_timestamp,
            Some("2026-03-28T10:00:00Z".to_string())
        );
        assert!(pod.metadata.labels.is_some());
    }

    #[test]
    fn spec_status_passthrough() {
        let result = parse_list_response(SAMPLE_POD_LIST, "pods", "default").unwrap();
        let pod = &result.items[0];
        assert!(pod.spec.is_some());
        let status = pod.status.as_ref().unwrap();
        assert_eq!(status.get("phase").unwrap().as_str().unwrap(), "Running");
    }

    #[test]
    fn missing_spec_is_none() {
        let result = parse_list_response(SAMPLE_POD_LIST, "pods", "default").unwrap();
        // Second pod has no spec
        assert!(result.items[1].spec.is_none());
    }

    const SAMPLE_SINGLE_POD: &str = r#"{
        "apiVersion": "v1",
        "kind": "Pod",
        "metadata": {
            "name": "test-pod",
            "namespace": "kube-system"
        },
        "spec": {"nodeName": "node-1"},
        "status": {"phase": "Succeeded"}
    }"#;

    #[test]
    fn parse_single_resource() {
        let result = parse_get_response(SAMPLE_SINGLE_POD).unwrap();
        assert_eq!(result.resource.kind, "Pod");
        assert_eq!(result.resource.metadata.name, "test-pod");
        assert_eq!(
            result.resource.metadata.namespace,
            Some("kube-system".to_string())
        );
    }

    #[test]
    fn invalid_json_errors() {
        let result = parse_list_response("not json", "pods", "default");
        assert!(result.is_err());
    }

    #[test]
    fn empty_list() {
        let json = r#"{"apiVersion":"v1","kind":"PodList","items":[]}"#;
        let result = parse_list_response(json, "pods", "default").unwrap();
        assert_eq!(result.count, 0);
        assert!(result.items.is_empty());
    }
}
