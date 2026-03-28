use rusqlite::{Connection, OpenFlags};
use serde::Serialize;
use serde_json::Value;

/// Result of a SQL query.
#[derive(Debug, Serialize, Clone)]
pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Value>>,
    pub row_count: u64,
}

/// Database schema info.
#[derive(Debug, Serialize, Clone)]
pub struct TableInfo {
    pub name: String,
    pub columns: Vec<ColumnInfo>,
}

#[derive(Debug, Serialize, Clone)]
pub struct ColumnInfo {
    pub name: String,
    pub data_type: String,
    pub not_null: bool,
    pub primary_key: bool,
}

/// Allowed database paths — populated from CLI flags at startup.
/// For now, we validate at query time.
static ALLOWED_PATHS: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();

/// Set the allowed database paths. Called once at startup from --allow-db-path flags.
#[allow(dead_code)]
pub fn set_allowed_paths(paths: Vec<String>) {
    let _ = ALLOWED_PATHS.set(paths);
}

/// Get the list of allowed paths.
pub fn get_allowed_paths() -> &'static [String] {
    ALLOWED_PATHS.get().map(|v| v.as_slice()).unwrap_or(&[])
}

/// Validate that a database path is allowed.
fn validate_path(db_path: &str) -> Result<String, String> {
    let allowed = get_allowed_paths();

    // If no paths configured, allow any path under $HOME or /tmp (dev convenience).
    // In production, --allow-db-path flags should be used.
    if allowed.is_empty() {
        let canonical = std::fs::canonicalize(db_path)
            .map_err(|e| format!("path error: {e}"))?;
        let canonical_str = canonical.to_string_lossy().to_string();
        if let Ok(home) = std::env::var("HOME") {
            if canonical_str.starts_with(&home) {
                return Ok(canonical_str);
            }
        }
        if canonical_str.starts_with("/tmp") {
            return Ok(canonical_str);
        }
        return Err("no databases configured and path is outside $HOME".to_string());
    }

    let canonical = std::fs::canonicalize(db_path)
        .map_err(|e| format!("path error: {e}"))?;
    let canonical_str = canonical.to_string_lossy().to_string();

    for allowed_path in allowed {
        let allowed_canonical = std::fs::canonicalize(allowed_path)
            .map_err(|e| format!("allowed path error: {e}"))?;
        if canonical_str.starts_with(&allowed_canonical.to_string_lossy().to_string()) {
            return Ok(canonical_str);
        }
    }

    Err(format!(
        "path '{}' is not in the allowed list. Start server with --allow-db-path",
        db_path
    ))
}

/// Open a database in read-only mode.
fn open_db(path: &str) -> Result<Connection, String> {
    let validated = validate_path(path)?;
    Connection::open_with_flags(
        &validated,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| format!("failed to open database: {e}"))
}

/// Execute a read-only SQL query and return structured results.
pub fn query(db_path: &str, sql: &str) -> Result<QueryResult, String> {
    // Block obviously destructive statements
    let upper = sql.trim().to_uppercase();
    if upper.starts_with("INSERT")
        || upper.starts_with("UPDATE")
        || upper.starts_with("DELETE")
        || upper.starts_with("DROP")
        || upper.starts_with("ALTER")
        || upper.starts_with("CREATE")
        || upper.starts_with("TRUNCATE")
    {
        return Err("write operations are not allowed in read-only mode".to_string());
    }

    let conn = open_db(db_path)?;

    // Set query timeout to 5 seconds
    conn.busy_timeout(std::time::Duration::from_secs(5))
        .map_err(|e| format!("timeout config error: {e}"))?;

    let mut stmt = conn.prepare(sql)
        .map_err(|e| format!("SQL error: {e}"))?;

    let columns: Vec<String> = stmt.column_names().iter().map(|s| s.to_string()).collect();
    let col_count = columns.len();

    let rows: Vec<Vec<Value>> = stmt
        .query_map([], |row| {
            let mut values = Vec::with_capacity(col_count);
            for i in 0..col_count {
                let val: Value = match row.get_ref(i) {
                    Ok(rusqlite::types::ValueRef::Null) => Value::Null,
                    Ok(rusqlite::types::ValueRef::Integer(n)) => Value::Number(n.into()),
                    Ok(rusqlite::types::ValueRef::Real(f)) => {
                        serde_json::Number::from_f64(f)
                            .map(Value::Number)
                            .unwrap_or(Value::Null)
                    }
                    Ok(rusqlite::types::ValueRef::Text(s)) => {
                        Value::String(String::from_utf8_lossy(s).to_string())
                    }
                    Ok(rusqlite::types::ValueRef::Blob(b)) => {
                        Value::String(format!("<blob {} bytes>", b.len()))
                    }
                    Err(_) => Value::Null,
                };
                values.push(val);
            }
            Ok(values)
        })
        .map_err(|e| format!("query error: {e}"))?
        .filter_map(|r| r.ok())
        .collect();

    let row_count = rows.len() as u64;

    Ok(QueryResult {
        columns,
        rows,
        row_count,
    })
}

/// List tables in a database.
pub fn list_tables(db_path: &str) -> Result<Vec<TableInfo>, String> {
    let conn = open_db(db_path)?;

    let mut stmt = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
        .map_err(|e| format!("SQL error: {e}"))?;

    let table_names: Vec<String> = stmt
        .query_map([], |row| row.get(0))
        .map_err(|e| format!("query error: {e}"))?
        .filter_map(|r| r.ok())
        .collect();

    let mut tables = Vec::new();
    for table_name in table_names {
        let mut info_stmt = conn
            .prepare(&format!("PRAGMA table_info(\"{}\")", table_name))
            .map_err(|e| format!("pragma error: {e}"))?;

        let columns: Vec<ColumnInfo> = info_stmt
            .query_map([], |row| {
                Ok(ColumnInfo {
                    name: row.get(1)?,
                    data_type: row.get(2)?,
                    not_null: row.get::<_, i32>(3)? != 0,
                    primary_key: row.get::<_, i32>(5)? != 0,
                })
            })
            .map_err(|e| format!("pragma query error: {e}"))?
            .filter_map(|r| r.ok())
            .collect();

        tables.push(TableInfo {
            name: table_name,
            columns,
        });
    }

    Ok(tables)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_db() -> (tempfile::TempDir, String) {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("test.db");
        let path_str = path.to_str().unwrap().to_string();

        let conn = Connection::open(&path).unwrap();
        conn.execute_batch(
            "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL, email TEXT);
             INSERT INTO users VALUES (1, 'Alice', 'alice@example.com');
             INSERT INTO users VALUES (2, 'Bob', NULL);"
        ).unwrap();

        (dir, path_str)
    }

    #[test]
    fn query_returns_structured() {
        let (_dir, path) = create_test_db();
        let result = query(&path, "SELECT id, name, email FROM users ORDER BY id").unwrap();
        assert_eq!(result.columns, vec!["id", "name", "email"]);
        assert_eq!(result.row_count, 2);
        assert_eq!(result.rows[0][0], Value::Number(1.into()));
        assert_eq!(result.rows[0][1], Value::String("Alice".to_string()));
        assert_eq!(result.rows[1][2], Value::Null);
    }

    #[test]
    fn blocks_write_operations() {
        let (_dir, path) = create_test_db();
        assert!(query(&path, "INSERT INTO users VALUES (3, 'Eve', 'e@e.com')").is_err());
        assert!(query(&path, "DELETE FROM users").is_err());
        assert!(query(&path, "DROP TABLE users").is_err());
        assert!(query(&path, "UPDATE users SET name='X'").is_err());
    }

    #[test]
    fn list_tables_works() {
        let (_dir, path) = create_test_db();
        let tables = list_tables(&path).unwrap();
        assert_eq!(tables.len(), 1);
        assert_eq!(tables[0].name, "users");
        assert_eq!(tables[0].columns.len(), 3);
        assert!(tables[0].columns[0].primary_key);
    }

    #[test]
    fn nonexistent_db_errors() {
        let result = query("/tmp/nonexistent-mcp-test-db.sqlite", "SELECT 1");
        assert!(result.is_err());
    }
}
