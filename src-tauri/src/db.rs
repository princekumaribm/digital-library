use crate::error::ApiResult;
use crate::util::{hash_password, iso, now_utc, DEFAULT_PASSWORD, DEFAULT_USERNAME};
use rusqlite::types::{Value as Sql, ValueRef};
use rusqlite::{params, params_from_iter, Connection};
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

pub const SETTINGS_DEFAULTS: [(&str, &str); 3] =
    [("library_name", "Digital Library"), ("currency", "₹"), ("tax_percent", "0")];

// Same schema as the original Python version, so existing databases and backups keep working.
const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS users (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    username TEXT NOT NULL UNIQUE COLLATE NOCASE,
    password_hash TEXT NOT NULL,
    default_password INTEGER NOT NULL DEFAULT 1,
    updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS members (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    first_name TEXT NOT NULL,
    last_name TEXT NOT NULL,
    email TEXT NOT NULL,
    phone TEXT NOT NULL,
    id_number TEXT NOT NULL UNIQUE COLLATE NOCASE,
    notes TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS computers (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE COLLATE NOCASE,
    location TEXT NOT NULL DEFAULT '',
    active INTEGER NOT NULL DEFAULT 1
);
CREATE TABLE IF NOT EXISTS rate_plans (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE COLLATE NOCASE,
    description TEXT NOT NULL DEFAULT '',
    rate_per_hour REAL NOT NULL,
    minimum_minutes INTEGER NOT NULL DEFAULT 0,
    increment_minutes INTEGER NOT NULL DEFAULT 1,
    max_charge REAL NOT NULL DEFAULT 0,
    is_default INTEGER NOT NULL DEFAULT 0,
    active INTEGER NOT NULL DEFAULT 1
);
CREATE TABLE IF NOT EXISTS sessions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    member_id INTEGER NOT NULL REFERENCES members(id),
    computer_id INTEGER REFERENCES computers(id) ON DELETE SET NULL,
    computer_name TEXT NOT NULL DEFAULT '',
    rate_plan_id INTEGER REFERENCES rate_plans(id) ON DELETE SET NULL,
    plan_name TEXT NOT NULL,
    rate_per_hour REAL NOT NULL,
    minimum_minutes INTEGER NOT NULL,
    increment_minutes INTEGER NOT NULL,
    max_charge REAL NOT NULL,
    tax_percent REAL NOT NULL,
    check_in TEXT NOT NULL,
    check_out TEXT,
    duration_minutes INTEGER,
    billable_minutes INTEGER,
    subtotal REAL,
    discount REAL,
    tax REAL,
    total REAL,
    payment_method TEXT,
    notes TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'completed', 'cancelled'))
);
CREATE UNIQUE INDEX IF NOT EXISTS one_active_session_per_member ON sessions(member_id) WHERE status = 'active';
CREATE UNIQUE INDEX IF NOT EXISTS one_active_session_per_computer ON sessions(computer_id) WHERE status = 'active';
CREATE INDEX IF NOT EXISTS sessions_check_in ON sessions(check_in);
CREATE INDEX IF NOT EXISTS sessions_check_out ON sessions(check_out);
"#;

pub fn connect(path: &Path) -> ApiResult<Connection> {
    let conn = Connection::open(path)?;
    conn.busy_timeout(Duration::from_secs(15))?;
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;
    Ok(conn)
}

pub fn init(path: &Path) -> ApiResult<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut conn = connect(path)?;
    conn.query_row("PRAGMA journal_mode = WAL", [], |_| Ok(()))?;
    let tx = conn.transaction()?;
    tx.execute_batch(SCHEMA)?;
    for (key, value) in SETTINGS_DEFAULTS {
        tx.execute("INSERT OR IGNORE INTO settings (key, value) VALUES (?1, ?2)", params![key, value])?;
    }
    let has_user: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM users)", [], |r| r.get(0))?;
    if !has_user {
        tx.execute(
            "INSERT INTO users (username, password_hash, default_password, updated_at) VALUES (?1, ?2, 1, ?3)",
            params![DEFAULT_USERNAME, hash_password(DEFAULT_PASSWORD), iso(now_utc())],
        )?;
    }
    let seeded: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM settings WHERE key = 'seeded')", [], |r| r.get(0))?;
    if !seeded {
        for (name, description, rate, minimum, increment, is_default) in [
            ("Standard", "Regular members", 60.0, 30, 15, 1),
            ("Student", "Students with valid ID", 40.0, 30, 15, 0),
            ("Senior Citizen", "Members aged 60+", 30.0, 15, 15, 0),
        ] {
            tx.execute(
                "INSERT OR IGNORE INTO rate_plans (name, description, rate_per_hour, minimum_minutes, \
                 increment_minutes, max_charge, is_default) VALUES (?1, ?2, ?3, ?4, ?5, 0, ?6)",
                params![name, description, rate, minimum, increment, is_default],
            )?;
        }
        for i in 1..=6 {
            tx.execute(
                "INSERT OR IGNORE INTO computers (name, location) VALUES (?1, 'Main hall')",
                params![format!("PC-{i:02}")],
            )?;
        }
        tx.execute("INSERT INTO settings (key, value) VALUES ('seeded', '1')", [])?;
    }
    tx.commit()?;
    Ok(())
}

pub fn rows(conn: &Connection, sql: &str, args: &[Sql]) -> ApiResult<Vec<Value>> {
    let mut stmt = conn.prepare(sql)?;
    let names: Vec<String> = stmt.column_names().iter().map(|n| n.to_string()).collect();
    let mut query = stmt.query(params_from_iter(args.iter()))?;
    let mut out = Vec::new();
    while let Some(row) = query.next()? {
        let mut map = Map::new();
        for (i, name) in names.iter().enumerate() {
            let value = match row.get_ref(i)? {
                ValueRef::Null | ValueRef::Blob(_) => Value::Null,
                ValueRef::Integer(n) => json!(n),
                ValueRef::Real(f) => json!(f),
                ValueRef::Text(t) => Value::String(String::from_utf8_lossy(t).into_owned()),
            };
            map.insert(name.clone(), value);
        }
        out.push(Value::Object(map));
    }
    Ok(out)
}

pub fn row(conn: &Connection, sql: &str, args: &[Sql]) -> ApiResult<Option<Value>> {
    Ok(rows(conn, sql, args)?.into_iter().next())
}

pub fn one_i64(conn: &Connection, sql: &str, args: &[Sql]) -> ApiResult<i64> {
    Ok(conn.query_row(sql, params_from_iter(args.iter()), |r| r.get::<_, Option<i64>>(0))?.unwrap_or(0))
}

pub fn one_f64(conn: &Connection, sql: &str, args: &[Sql]) -> ApiResult<f64> {
    Ok(conn.query_row(sql, params_from_iter(args.iter()), |r| r.get::<_, Option<f64>>(0))?.unwrap_or(0.0))
}

pub fn load_settings(conn: &Connection) -> ApiResult<HashMap<String, String>> {
    let mut settings: HashMap<String, String> =
        SETTINGS_DEFAULTS.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
    let mut stmt = conn.prepare("SELECT key, value FROM settings")?;
    let pairs = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
    for pair in pairs {
        let (key, value) = pair?;
        settings.insert(key, value);
    }
    Ok(settings)
}

pub fn tax_percent(settings: &HashMap<String, String>) -> f64 {
    settings.get("tax_percent").and_then(|v| v.parse().ok()).unwrap_or(0.0)
}

pub fn t(value: impl Into<String>) -> Sql {
    Sql::Text(value.into())
}

pub fn i(value: i64) -> Sql {
    Sql::Integer(value)
}
