use crate::harness::Server;
use rusqlite::Connection;
use serde_json::json;
use tempfile::TempDir;

fn make_db(dir: &TempDir) -> std::path::PathBuf {
    let path = dir.path().join("test.db");
    let conn = Connection::open(&path).unwrap();
    conn.execute_batch(
        "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL, email TEXT, age INTEGER);
         INSERT INTO users VALUES (1, 'Alice', 'alice@test.com', 30);
         INSERT INTO users VALUES (2, 'Bob', NULL, 25);
         INSERT INTO users VALUES (3, 'Carol', 'carol@test.com', 35);
         CREATE TABLE posts (id INTEGER PRIMARY KEY, user_id INTEGER, title TEXT);
         INSERT INTO posts VALUES (1, 1, 'Hello');
         INSERT INTO posts VALUES (2, 1, 'World');
         INSERT INTO posts VALUES (3, 2, 'Foo');",
    )
    .unwrap();
    path
}

#[test]
fn list_tables_returns_schema() {
    let d = TempDir::new().unwrap();
    let p = make_db(&d);
    let mut s = Server::spawn(&[]);
    let r = s.call("sqlite_tables", json!({"db_path": p}));
    assert!(r.success());
    let tables = r.data.as_array().unwrap();
    assert_eq!(tables.len(), 2);
    assert!(tables.iter().any(|t| t["name"] == "users"));
}

#[test]
fn primary_key_detected() {
    let d = TempDir::new().unwrap();
    let p = make_db(&d);
    let mut s = Server::spawn(&[]);
    let r = s.call("sqlite_tables", json!({"db_path": p}));
    assert!(r.success());
    let users = r
        .data
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == "users")
        .unwrap();
    let cols = users["columns"].as_array().unwrap();
    assert!(cols.iter().any(|c| c["primary_key"].as_bool().unwrap()));
}

#[test]
fn select_returns_typed_rows() {
    let d = TempDir::new().unwrap();
    let p = make_db(&d);
    let mut s = Server::spawn(&[]);
    let r = s.call(
        "sqlite_query",
        json!({
            "db_path": p,
            "sql": "SELECT id, name, age FROM users ORDER BY id"
        }),
    );
    assert!(r.success());
    assert_eq!(r.data["row_count"], 3);
    assert_eq!(r.data["columns"], json!(["id", "name", "age"]));
    assert_eq!(r.data["rows"][0][1], "Alice");
}

#[test]
fn null_values_serialize_as_null() {
    let d = TempDir::new().unwrap();
    let p = make_db(&d);
    let mut s = Server::spawn(&[]);
    let r = s.call(
        "sqlite_query",
        json!({
            "db_path": p,
            "sql": "SELECT email FROM users WHERE id = 2"
        }),
    );
    assert!(r.success());
    assert!(r.data["rows"][0][0].is_null());
}

#[test]
fn join_returns_correct_rows() {
    let d = TempDir::new().unwrap();
    let p = make_db(&d);
    let mut s = Server::spawn(&[]);
    let r = s.call("sqlite_query", json!({
        "db_path": p,
        "sql": "SELECT u.name, COUNT(p.id) FROM users u LEFT JOIN posts p ON u.id = p.user_id GROUP BY u.id ORDER BY u.id"
    }));
    assert!(r.success());
    assert_eq!(r.data["row_count"], 3);
}

#[test]
fn insert_blocked() {
    let d = TempDir::new().unwrap();
    let p = make_db(&d);
    let mut s = Server::spawn(&[]);
    let r = s.call(
        "sqlite_query",
        json!({
            "db_path": p,
            "sql": "INSERT INTO users VALUES (99, 'Hacker', 'h@x.com', 0)"
        }),
    );
    assert!(!r.success());
}

#[test]
fn update_blocked() {
    let d = TempDir::new().unwrap();
    let p = make_db(&d);
    let mut s = Server::spawn(&[]);
    let r = s.call(
        "sqlite_query",
        json!({
            "db_path": p,
            "sql": "UPDATE users SET name = 'evil'"
        }),
    );
    assert!(!r.success());
}

#[test]
fn delete_blocked() {
    let d = TempDir::new().unwrap();
    let p = make_db(&d);
    let mut s = Server::spawn(&[]);
    let r = s.call(
        "sqlite_query",
        json!({
            "db_path": p,
            "sql": "DELETE FROM users"
        }),
    );
    assert!(!r.success());
}

#[test]
fn drop_blocked() {
    let d = TempDir::new().unwrap();
    let p = make_db(&d);
    let mut s = Server::spawn(&[]);
    let r = s.call(
        "sqlite_query",
        json!({
            "db_path": p,
            "sql": "DROP TABLE users"
        }),
    );
    assert!(!r.success());
}

#[test]
fn invalid_sql_errors() {
    let d = TempDir::new().unwrap();
    let p = make_db(&d);
    let mut s = Server::spawn(&[]);
    let r = s.call(
        "sqlite_query",
        json!({
            "db_path": p,
            "sql": "SELECTT broken"
        }),
    );
    assert!(!r.success());
}

#[test]
fn nonexistent_db_errors() {
    let mut s = Server::spawn(&[]);
    let r = s.call(
        "sqlite_query",
        json!({
            "db_path": "/no/such/db.sqlite",
            "sql": "SELECT 1"
        }),
    );
    assert!(!r.success());
}

#[test]
fn empty_result_set() {
    let d = TempDir::new().unwrap();
    let p = make_db(&d);
    let mut s = Server::spawn(&[]);
    let r = s.call(
        "sqlite_query",
        json!({
            "db_path": p,
            "sql": "SELECT * FROM users WHERE id = 999"
        }),
    );
    assert!(r.success());
    assert_eq!(r.data["row_count"], 0);
}

#[test]
fn aggregate_query() {
    let d = TempDir::new().unwrap();
    let p = make_db(&d);
    let mut s = Server::spawn(&[]);
    let r = s.call(
        "sqlite_query",
        json!({
            "db_path": p,
            "sql": "SELECT COUNT(*) AS n, AVG(age) AS avg_age FROM users"
        }),
    );
    assert!(r.success());
    assert_eq!(r.data["rows"][0][0], 3);
}

#[test]
fn nonexistent_table_errors() {
    let d = TempDir::new().unwrap();
    let p = make_db(&d);
    let mut s = Server::spawn(&[]);
    let r = s.call(
        "sqlite_query",
        json!({
            "db_path": p,
            "sql": "SELECT * FROM no_such_table"
        }),
    );
    assert!(!r.success());
}
