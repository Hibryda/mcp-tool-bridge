//! Curl tool — structured HTTP responses with typed status, headers, timing.

use bridge_core::BridgeError;
use serde::Serialize;
use std::collections::HashMap;

/// Structured HTTP response.
#[derive(Debug, Serialize, Clone)]
pub struct HttpResponse {
    pub status_code: u16,
    pub status_text: String,
    pub headers: HashMap<String, String>,
    pub content_type: Option<String>,
    pub body: String,
    /// Whether the body was valid JSON (parsed successfully).
    pub body_is_json: bool,
    pub timing: HttpTiming,
    pub size_bytes: u64,
    pub redirect_count: u32,
    pub effective_url: String,
}

/// Timing breakdown from curl -w.
#[derive(Debug, Serialize, Clone)]
pub struct HttpTiming {
    pub dns_ms: f64,
    pub connect_ms: f64,
    pub tls_ms: f64,
    pub first_byte_ms: f64,
    pub total_ms: f64,
}

/// Run curl and return structured response.
pub async fn http_request(
    url: &str,
    method: &str,
    headers: &[(String, String)],
    body: Option<&str>,
    follow_redirects: bool,
    timeout_secs: u64,
) -> Result<HttpResponse, BridgeError> {
    let mut args = vec![
        "-s".to_string(),       // silent
        "-S".to_string(),       // show errors
        "-D".to_string(), "-".to_string(), // dump headers to stdout
        "-w".to_string(),       // write-out format
        "\n__CURL_TIMING__\nstatus_code:%{http_code}\neffective_url:%{url_effective}\nredirect_count:%{num_redirects}\ntime_namelookup:%{time_namelookup}\ntime_connect:%{time_connect}\ntime_appconnect:%{time_appconnect}\ntime_starttransfer:%{time_starttransfer}\ntime_total:%{time_total}\nsize_download:%{size_download}\n".to_string(),
        "-X".to_string(), method.to_uppercase(),
        "--max-time".to_string(), timeout_secs.to_string(),
    ];

    if follow_redirects {
        args.push("-L".to_string());
        args.push("--max-redirs".to_string());
        args.push("10".to_string());
    }

    for (name, value) in headers {
        args.push("-H".to_string());
        args.push(format!("{name}: {value}"));
    }

    if let Some(data) = body {
        args.push("-d".to_string());
        args.push(data.to_string());
    }

    args.push(url.to_string());

    let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let output = bridge_core::run_command("curl", &arg_refs).await?;

    parse_curl_output(&output, url)
}

/// Parse curl's combined header + body + timing output.
fn parse_curl_output(output: &str, original_url: &str) -> Result<HttpResponse, BridgeError> {
    // Split at __CURL_TIMING__ marker
    let (response_part, timing_part) = output
        .rsplit_once("\n__CURL_TIMING__")
        .unwrap_or((output, ""));

    // Parse timing variables
    let timing_vars = parse_timing_vars(timing_part);

    // Split response into headers and body at first double newline
    let (header_section, body) = split_headers_body(response_part);

    // Parse status line and headers
    let (status_code, status_text) = parse_status_line(&header_section);
    let headers = parse_headers(&header_section);
    let content_type = headers.get("content-type").cloned();

    // Check if body is valid JSON
    let body_is_json = serde_json::from_str::<serde_json::Value>(&body).is_ok();

    let timing = HttpTiming {
        dns_ms: timing_vars.get("time_namelookup").copied().unwrap_or(0.0) * 1000.0,
        connect_ms: timing_vars.get("time_connect").copied().unwrap_or(0.0) * 1000.0,
        tls_ms: timing_vars.get("time_appconnect").copied().unwrap_or(0.0) * 1000.0,
        first_byte_ms: timing_vars
            .get("time_starttransfer")
            .copied()
            .unwrap_or(0.0)
            * 1000.0,
        total_ms: timing_vars.get("time_total").copied().unwrap_or(0.0) * 1000.0,
    };

    Ok(HttpResponse {
        status_code: timing_vars
            .get("status_code")
            .map(|v| *v as u16)
            .unwrap_or(status_code),
        status_text,
        headers,
        content_type,
        body,
        body_is_json,
        timing,
        size_bytes: timing_vars.get("size_download").copied().unwrap_or(0.0) as u64,
        redirect_count: timing_vars.get("redirect_count").copied().unwrap_or(0.0) as u32,
        effective_url: timing_vars
            .get("effective_url")
            .map(|_| {
                // effective_url is a string, not a float — extract from raw
                timing_part
                    .lines()
                    .find(|l| l.starts_with("effective_url:"))
                    .and_then(|l| l.strip_prefix("effective_url:"))
                    .unwrap_or(original_url)
                    .to_string()
            })
            .unwrap_or_else(|| original_url.to_string()),
    })
}

fn parse_timing_vars(timing: &str) -> HashMap<String, f64> {
    let mut vars = HashMap::new();
    for line in timing.lines() {
        if let Some((key, value)) = line.split_once(':') {
            let key = key.trim();
            let value = value.trim();
            if let Ok(num) = value.parse::<f64>() {
                vars.insert(key.to_string(), num);
            } else {
                // Store non-numeric values as sentinel (for effective_url)
                vars.insert(key.to_string(), f64::NAN);
            }
        }
    }
    vars
}

fn split_headers_body(response: &str) -> (String, String) {
    // Headers end at double CRLF or double LF
    if let Some(pos) = response.find("\r\n\r\n") {
        let headers = response[..pos].to_string();
        let body = response[pos + 4..].to_string();
        (headers, body)
    } else if let Some(pos) = response.find("\n\n") {
        let headers = response[..pos].to_string();
        let body = response[pos + 2..].to_string();
        (headers, body)
    } else {
        // No clear separation — treat entire thing as body
        (String::new(), response.to_string())
    }
}

fn parse_status_line(header_section: &str) -> (u16, String) {
    // Find the last HTTP status line (for redirects, there may be multiple)
    let status_line = header_section
        .lines()
        .rev()
        .find(|l| l.starts_with("HTTP/"))
        .unwrap_or("");

    // Parse "HTTP/1.1 200 OK" or "HTTP/2 404 Not Found"
    let parts: Vec<&str> = status_line.splitn(3, ' ').collect();
    let code = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
    let text = parts.get(2).unwrap_or(&"").to_string();
    (code, text)
}

fn parse_headers(header_section: &str) -> HashMap<String, String> {
    let mut headers = HashMap::new();
    for line in header_section.lines() {
        // Skip status lines
        if line.starts_with("HTTP/") {
            continue;
        }
        if let Some((name, value)) = line.split_once(':') {
            headers.insert(name.trim().to_lowercase(), value.trim().to_string());
        }
    }
    headers
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_response() {
        let output = "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: 13\r\n\r\n{\"ok\": true}\n__CURL_TIMING__\nstatus_code:200\ntime_total:0.050\nsize_download:13\n";
        let result = parse_curl_output(output, "http://example.com").unwrap();
        assert_eq!(result.status_code, 200);
        assert_eq!(result.status_text, "OK");
        assert!(result.body_is_json);
        assert_eq!(result.body, "{\"ok\": true}");
        assert!(result.timing.total_ms > 0.0);
    }

    #[test]
    fn parse_headers_lowercase() {
        let output = "HTTP/1.1 404 Not Found\r\nX-Custom: hello\r\nContent-Type: text/plain\r\n\r\nnot found\n__CURL_TIMING__\nstatus_code:404\ntime_total:0.010\nsize_download:9\n";
        let result = parse_curl_output(output, "http://example.com").unwrap();
        assert_eq!(result.status_code, 404);
        assert_eq!(result.headers.get("x-custom"), Some(&"hello".to_string()));
        assert!(!result.body_is_json);
    }

    #[test]
    fn parse_timing() {
        let output = "HTTP/1.1 200 OK\r\n\r\nok\n__CURL_TIMING__\nstatus_code:200\ntime_namelookup:0.001\ntime_connect:0.005\ntime_appconnect:0.020\ntime_starttransfer:0.025\ntime_total:0.030\nsize_download:2\n";
        let result = parse_curl_output(output, "http://example.com").unwrap();
        assert!((result.timing.dns_ms - 1.0).abs() < 0.1);
        assert!((result.timing.connect_ms - 5.0).abs() < 0.1);
        assert!((result.timing.tls_ms - 20.0).abs() < 0.1);
        assert!((result.timing.total_ms - 30.0).abs() < 0.1);
    }

    #[test]
    fn non_json_body() {
        let output = "HTTP/1.1 200 OK\r\n\r\nplain text body\n__CURL_TIMING__\nstatus_code:200\ntime_total:0.001\nsize_download:15\n";
        let result = parse_curl_output(output, "http://test.com").unwrap();
        assert!(!result.body_is_json);
        assert_eq!(result.body, "plain text body");
    }
}
