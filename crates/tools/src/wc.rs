use bridge_core::WcResult;

/// Count lines, words, bytes, and chars in a file.
pub async fn word_count(path: &str) -> Result<WcResult, bridge_core::BridgeError> {
    let content = tokio::fs::read(path).await?;
    let text = String::from_utf8_lossy(&content);

    let lines = text.lines().count() as u64;
    let words = text.split_whitespace().count() as u64;
    let bytes = content.len() as u64;
    let chars = text.chars().count() as u64;

    Ok(WcResult {
        file: Some(path.to_string()),
        lines,
        words,
        bytes,
        chars,
    })
}

/// Count lines, words, bytes, and chars from a string (stdin equivalent).
pub fn word_count_str(input: &str) -> WcResult {
    let lines = input.lines().count() as u64;
    let words = input.split_whitespace().count() as u64;
    let bytes = input.len() as u64;
    let chars = input.chars().count() as u64;

    WcResult {
        file: None,
        lines,
        words,
        bytes,
        chars,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_string() {
        let r = word_count_str("");
        assert_eq!(r.lines, 0);
        assert_eq!(r.words, 0);
        assert_eq!(r.bytes, 0);
        assert_eq!(r.chars, 0);
    }

    #[test]
    fn single_line() {
        let r = word_count_str("hello world");
        assert_eq!(r.lines, 1);
        assert_eq!(r.words, 2);
        assert_eq!(r.bytes, 11);
        assert_eq!(r.chars, 11);
    }

    #[test]
    fn multi_line() {
        let r = word_count_str("line one\nline two\nline three");
        assert_eq!(r.lines, 3);
        assert_eq!(r.words, 6);
    }

    #[test]
    fn unicode() {
        let r = word_count_str("héllo wörld");
        assert_eq!(r.words, 2);
        assert_eq!(r.chars, 11);
        assert!(r.bytes > r.chars); // multi-byte chars
    }

    #[tokio::test]
    async fn file_count() {
        let dir = std::env::temp_dir().join("mcp-wc-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test.txt");
        std::fs::write(&path, "one two three\nfour five\n").unwrap();

        let r = word_count(path.to_str().unwrap()).await.unwrap();
        assert_eq!(r.lines, 2);
        assert_eq!(r.words, 5);
        assert_eq!(r.bytes, 24);
        assert!(r.file.is_some());

        std::fs::remove_dir_all(&dir).ok();
    }
}
