use crate::db::{self, i, load_settings, one_f64, one_i64, row, rows, t, tax_percent};
use crate::error::{ApiError, ApiResult};
use crate::util::*;
use chrono::{DateTime, Duration as ChronoDuration, SecondsFormat, Utc};
use regex::Regex;
use rusqlite::backup::Progress;
use rusqlite::types::Value as Sql;
use rusqlite::{params, Connection, DatabaseName};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::DialogExt;

const APP_ID: &str = "digital-library";
const PAYMENT_METHODS: [&str; 5] = ["Cash", "Card", "UPI", "Bank transfer", "Other"];
const SESSION_TTL: Duration = Duration::from_secs(12 * 3600);
const MAX_FAILED_LOGINS: u32 = 5;
const LOCKOUT: Duration = Duration::from_secs(60);
const SQLITE_MAGIC: &[u8] = b"SQLite format 3\0";
const REQUIRED_TABLES: [&str; 5] = ["settings", "members", "computers", "rate_plans", "sessions"];

static EMAIL_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[^@\s]+@[^@\s]+\.[^@\s]+$").unwrap());
static PHONE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[0-9+()\-.\s]{6,25}$").unwrap());

// ---------------------------------------------------------------- state

#[derive(Default)]
struct AuthState {
    user: Option<(String, Instant)>,
    failures: u32,
    locked_until: Option<Instant>,
}

pub struct AppState {
    pub data_dir: PathBuf,
    pub db_path: PathBuf,
    auth: Mutex<AuthState>,
}

impl AppState {
    pub fn new(data_dir: PathBuf) -> Self {
        let db_path = data_dir.join("library.db");
        Self { data_dir, db_path, auth: Mutex::new(AuthState::default()) }
    }

    fn auth(&self) -> std::sync::MutexGuard<'_, AuthState> {
        self.auth.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn current_user(&self) -> Option<String> {
        let mut auth = self.auth();
        let active = auth
            .user
            .as_ref()
            .filter(|(_, last)| last.elapsed() < SESSION_TTL)
            .map(|(name, _)| name.clone());
        auth.user = active.clone().map(|name| (name, Instant::now()));
        active
    }

    fn sign_in(&self, username: String) {
        self.auth().user = Some((username, Instant::now()));
    }

    fn sign_out(&self) {
        self.auth().user = None;
    }

    fn check_lockout(&self) -> ApiResult<()> {
        if let Some(until) = self.auth().locked_until {
            let now = Instant::now();
            if until > now {
                let wait = (until - now).as_secs_f64().ceil();
                return Err(ApiError::new(429, format!("Too many failed attempts. Try again in {wait} seconds.")));
            }
        }
        Ok(())
    }

    fn record_attempt(&self, success: bool) {
        let mut auth = self.auth();
        if success {
            auth.failures = 0;
            return;
        }
        auth.failures += 1;
        if auth.failures >= MAX_FAILED_LOGINS {
            auth.failures = 0;
            auth.locked_until = Some(Instant::now() + LOCKOUT);
        }
    }
}

// ---------------------------------------------------------------- dispatch

struct Ctx<'a> {
    app: Option<&'a AppHandle>,
    state: &'a AppState,
    query: HashMap<String, String>,
    body: Value,
    user: Option<String>,
}

impl Ctx<'_> {
    fn conn(&self) -> ApiResult<Connection> {
        db::connect(&self.state.db_path)
    }

    fn q(&self, key: &str) -> String {
        query_param(&self.query, key)
    }

    fn app(&self) -> ApiResult<&AppHandle> {
        self.app.ok_or_else(|| ApiError::new(500, "This action needs the app window."))
    }

    fn version(&self) -> String {
        self.app
            .map(|app| app.package_info().version.to_string())
            .unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string())
    }
}

fn parse_id(segment: &str) -> ApiResult<i64> {
    segment.parse::<i64>().ok().filter(|n| *n > 0).ok_or_else(|| ApiError::new(404, "Not found."))
}

/// Routes a UI request. Paths and status codes match the original HTTP API so the UI barely changed.
pub fn dispatch(app: Option<&AppHandle>, state: &AppState, method: &str, full_path: &str, body: Value) -> ApiResult<Value> {
    if !body.is_object() {
        return Err(ApiError::new(400, "Expected a JSON object."));
    }
    let (path, query_string) = full_path.split_once('?').unwrap_or((full_path, ""));
    let query: HashMap<String, String> = form_urlencoded::parse(query_string.as_bytes()).into_owned().collect();
    let segments: Vec<&str> = path.trim_start_matches("/api/").trim_end_matches('/').split('/').collect();

    let public = matches!((method, segments.as_slice()), ("GET", ["health"]) | (_, ["auth", ..]));
    let user = state.current_user();
    if !public && user.is_none() {
        return Err(ApiError::new(401, "Please sign in."));
    }
    let ctx = Ctx { app, state, query, body, user };

    match (method, segments.as_slice()) {
        ("GET", ["health"]) => Ok(json!({"app": APP_ID, "version": ctx.version()})),
        ("GET", ["auth", "status"]) => auth_status(&ctx, ctx.user.as_deref()),
        ("POST", ["auth", "login"]) => login(&ctx),
        ("POST", ["auth", "logout"]) => {
            ctx.state.sign_out();
            Ok(json!({"ok": true}))
        }
        ("POST", ["auth", "change-password"]) => change_password(&ctx),

        ("GET", ["dashboard"]) => dashboard(&ctx),
        ("GET", ["settings"]) => get_settings(&ctx),
        ("PUT", ["settings"]) => update_settings(&ctx),

        ("GET", ["members"]) => list_members(&ctx),
        ("POST", ["members"]) => create_member(&ctx),
        ("GET", ["members", id]) => fetch_member(&ctx.conn()?, parse_id(id)?),
        ("PUT", ["members", id]) => update_member(&ctx, parse_id(id)?),
        ("DELETE", ["members", id]) => delete_member(&ctx, parse_id(id)?),

        ("GET", ["computers"]) => list_computers(&ctx),
        ("POST", ["computers"]) => create_computer(&ctx),
        ("PUT", ["computers", id]) => update_computer(&ctx, parse_id(id)?),
        ("DELETE", ["computers", id]) => delete_computer(&ctx, parse_id(id)?),

        ("GET", ["rates"]) => list_rates(&ctx),
        ("POST", ["rates"]) => create_rate(&ctx),
        ("PUT", ["rates", id]) => update_rate(&ctx, parse_id(id)?),
        ("DELETE", ["rates", id]) => delete_rate(&ctx, parse_id(id)?),

        ("GET", ["sessions"]) => list_sessions(&ctx),
        ("POST", ["sessions", "checkin"]) => check_in(&ctx),
        ("GET", ["sessions", id]) => fetch_session(&ctx.conn()?, parse_id(id)?),
        ("GET", ["sessions", id, "quote"]) => quote(&ctx, parse_id(id)?),
        ("POST", ["sessions", id, "checkout"]) => check_out(&ctx, parse_id(id)?),
        ("POST", ["sessions", id, "cancel"]) => cancel_session(&ctx, parse_id(id)?),

        ("GET", ["reports"]) => reports(&ctx),
        ("POST", ["export", kind]) => export_csv(&ctx, kind),
        ("POST", ["backup"]) => save_backup(&ctx),
        ("POST", ["restore"]) => restore_backup(&ctx),
        ("POST", ["print"]) => print_window(&ctx),

        _ => Err(ApiError::new(404, "Not found.")),
    }
}

// ---------------------------------------------------------------- authentication

fn auth_status(ctx: &Ctx, username: Option<&str>) -> ApiResult<Value> {
    let conn = ctx.conn()?;
    let library_name = load_settings(&conn)?.remove("library_name").unwrap_or_default();
    if let Some(name) = username {
        if let Some(user) = row(&conn, "SELECT username, default_password FROM users WHERE username = ?", &[t(name)])? {
            return Ok(json!({
                "authenticated": true,
                "library_name": library_name,
                "username": user["username"],
                "default_password": as_i64(&user["default_password"]) != 0,
            }));
        }
    }
    Ok(json!({"authenticated": false, "library_name": library_name}))
}

fn login(ctx: &Ctx) -> ApiResult<Value> {
    ctx.state.check_lockout()?;
    let username = text(&ctx.body, "username", "Username", true, 80)?;
    let pass = password(&ctx.body, "password", "Password")?;
    let conn = ctx.conn()?;
    let user = row(&conn, "SELECT username, password_hash FROM users WHERE username = ?", &[t(username)])?
        .filter(|u| verify_password(&pass, u["password_hash"].as_str().unwrap_or("")));
    ctx.state.record_attempt(user.is_some());
    let user = user.ok_or_else(|| ApiError::new(401, "Incorrect username or password."))?;
    let name = user["username"].as_str().unwrap_or_default().to_string();
    ctx.state.sign_in(name.clone());
    auth_status(ctx, Some(&name))
}

fn change_password(ctx: &Ctx) -> ApiResult<Value> {
    ctx.state.check_lockout()?;
    let username = match &ctx.user {
        Some(name) => name.clone(),
        None => text(&ctx.body, "username", "Username", true, 80)?,
    };
    let current = password(&ctx.body, "current_password", "Current password")?;
    let new = password(&ctx.body, "new_password", "New password")?;
    check_new_password(&new)?;
    let conn = ctx.conn()?;
    let user = row(&conn, "SELECT id, password_hash FROM users WHERE username = ?", &[t(username)])?;
    // By design, the default password also works here so a forgotten password can be reset.
    let ok = user.as_ref().is_some_and(|u| {
        is_default_password(&current) || verify_password(&current, u["password_hash"].as_str().unwrap_or(""))
    });
    ctx.state.record_attempt(ok);
    let user = user.filter(|_| ok).ok_or_else(|| ApiError::new(401, "The current password (or default password) is incorrect."))?;
    conn.execute(
        "UPDATE users SET password_hash = ?1, default_password = ?2, updated_at = ?3 WHERE id = ?4",
        params![hash_password(&new), i64::from(is_default_password(&new)), iso(now_utc()), as_i64(&user["id"])],
    )?;
    Ok(json!({"ok": true}))
}

// ---------------------------------------------------------------- dashboard & settings

fn dashboard(ctx: &Ctx) -> ApiResult<Value> {
    let conn = ctx.conn()?;
    let since = [t(iso(local_midnight_utc()))];
    Ok(json!({
        "active_sessions": one_i64(&conn, "SELECT COUNT(*) FROM sessions WHERE status = 'active'", &[])?,
        "computers_total": one_i64(&conn, "SELECT COUNT(*) FROM computers WHERE active = 1", &[])?,
        "computers_free": one_i64(&conn,
            "SELECT COUNT(*) FROM computers c WHERE c.active = 1 AND NOT EXISTS \
             (SELECT 1 FROM sessions s WHERE s.computer_id = c.id AND s.status = 'active')", &[])?,
        "members": one_i64(&conn, "SELECT COUNT(*) FROM members", &[])?,
        "today_checkins": one_i64(&conn,
            "SELECT COUNT(*) FROM sessions WHERE check_in >= ? AND status != 'cancelled'", &since)?,
        "today_completed": one_i64(&conn,
            "SELECT COUNT(*) FROM sessions WHERE status = 'completed' AND check_out >= ?", &since)?,
        "today_minutes": one_i64(&conn,
            "SELECT COALESCE(SUM(duration_minutes), 0) FROM sessions WHERE status = 'completed' AND check_out >= ?", &since)?,
        "today_revenue": money(one_f64(&conn,
            "SELECT COALESCE(SUM(total), 0) FROM sessions WHERE status = 'completed' AND check_out >= ?", &since)?),
    }))
}

fn get_settings(ctx: &Ctx) -> ApiResult<Value> {
    let settings = load_settings(&ctx.conn()?)?;
    Ok(json!({
        "library_name": settings["library_name"],
        "currency": settings["currency"],
        "tax_percent": tax_percent(&settings),
        "payment_methods": PAYMENT_METHODS,
        "data_dir": ctx.state.data_dir.display().to_string(),
        "version": ctx.version(),
    }))
}

fn update_settings(ctx: &Ctx) -> ApiResult<Value> {
    let body = &ctx.body;
    let mut updates: Vec<(&str, String)> = Vec::new();
    if body.get("library_name").is_some() {
        updates.push(("library_name", text(body, "library_name", "Library name", true, 80)?));
    }
    if body.get("currency").is_some() {
        updates.push(("currency", text(body, "currency", "Currency symbol", true, 5)?));
    }
    if body.get("tax_percent").is_some() {
        updates.push(("tax_percent", number(body, "tax_percent", "Tax percent", 0.0, 100.0, None)?.to_string()));
    }
    let conn = ctx.conn()?;
    for (key, value) in updates {
        conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
    }
    get_settings(ctx)
}

fn search_clause(columns: &[&str], term: &str) -> (String, Vec<Sql>) {
    let pattern = format!("%{}%", term.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_"));
    let clause = columns.iter().map(|c| format!("{c} LIKE ? ESCAPE '\\'")).collect::<Vec<_>>().join(" OR ");
    (format!("({clause})"), vec![t(pattern); columns.len()])
}

// ---------------------------------------------------------------- members

const MEMBER_SELECT: &str = "
SELECT m.*,
    (SELECT COUNT(*) FROM sessions s WHERE s.member_id = m.id AND s.status = 'completed') AS visits,
    (SELECT COALESCE(SUM(s.total), 0) FROM sessions s WHERE s.member_id = m.id AND s.status = 'completed') AS total_spent,
    (SELECT MAX(s.check_in) FROM sessions s WHERE s.member_id = m.id) AS last_visit,
    (SELECT s.id FROM sessions s WHERE s.member_id = m.id AND s.status = 'active') AS active_session_id
FROM members m
";

struct MemberInput {
    first_name: String,
    last_name: String,
    email: String,
    phone: String,
    id_number: String,
    notes: String,
}

fn member_input(body: &Value) -> ApiResult<MemberInput> {
    let m = MemberInput {
        first_name: text(body, "first_name", "First name", true, 80)?,
        last_name: text(body, "last_name", "Last name", true, 80)?,
        email: text(body, "email", "Email", true, 160)?.to_lowercase(),
        phone: text(body, "phone", "Phone", true, 25)?,
        id_number: text(body, "id_number", "ID number", true, 60)?,
        notes: text(body, "notes", "Notes", false, 1000)?,
    };
    if !EMAIL_RE.is_match(&m.email) {
        return Err(ApiError::new(400, "Enter a valid email address."));
    }
    if !PHONE_RE.is_match(&m.phone) || m.phone.chars().filter(|c| c.is_ascii_digit()).count() < 6 {
        return Err(ApiError::new(400, "Enter a valid phone number."));
    }
    Ok(m)
}

fn fetch_member(conn: &Connection, id: i64) -> ApiResult<Value> {
    row(conn, &format!("{MEMBER_SELECT} WHERE m.id = ?"), &[i(id)])?.ok_or_else(|| ApiError::new(404, "Member not found."))
}

fn list_members(ctx: &Ctx) -> ApiResult<Value> {
    let mut sql = MEMBER_SELECT.to_string();
    let mut args = Vec::new();
    let term = ctx.q("q");
    if !term.is_empty() {
        let (clause, extra) = search_clause(
            &["m.first_name", "m.last_name", "m.first_name || ' ' || m.last_name", "m.email", "m.phone", "m.id_number"],
            &term,
        );
        sql += &format!(" WHERE {clause}");
        args = extra;
    }
    sql += " ORDER BY m.first_name COLLATE NOCASE, m.last_name COLLATE NOCASE LIMIT ?";
    args.push(i(query_limit(&ctx.query, 1000, 5000)));
    Ok(json!({"items": rows(&ctx.conn()?, &sql, &args)?}))
}

fn create_member(ctx: &Ctx) -> ApiResult<Value> {
    let m = member_input(&ctx.body)?;
    let stamp = iso(now_utc());
    let conn = ctx.conn()?;
    conn.execute(
        "INSERT INTO members (first_name, last_name, email, phone, id_number, notes, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
        params![m.first_name, m.last_name, m.email, m.phone, m.id_number, m.notes, stamp],
    )?;
    fetch_member(&conn, conn.last_insert_rowid())
}

fn update_member(ctx: &Ctx, id: i64) -> ApiResult<Value> {
    let m = member_input(&ctx.body)?;
    let conn = ctx.conn()?;
    let changed = conn.execute(
        "UPDATE members SET first_name = ?1, last_name = ?2, email = ?3, phone = ?4, id_number = ?5, notes = ?6, \
         updated_at = ?7 WHERE id = ?8",
        params![m.first_name, m.last_name, m.email, m.phone, m.id_number, m.notes, iso(now_utc()), id],
    )?;
    if changed == 0 {
        return Err(ApiError::new(404, "Member not found."));
    }
    fetch_member(&conn, id)
}

fn delete_member(ctx: &Ctx, id: i64) -> ApiResult<Value> {
    let conn = ctx.conn()?;
    fetch_member(&conn, id)?;
    if one_i64(&conn, "SELECT EXISTS(SELECT 1 FROM sessions WHERE member_id = ?)", &[i(id)])? != 0 {
        return Err(ApiError::new(409, "This member has session history and cannot be deleted."));
    }
    conn.execute("DELETE FROM members WHERE id = ?1", params![id])?;
    Ok(json!({"ok": true}))
}

// ---------------------------------------------------------------- computers

const COMPUTER_SELECT: &str = "
SELECT c.*, s.id AS session_id, s.check_in AS session_check_in,
       m.first_name || ' ' || m.last_name AS session_member
FROM computers c
LEFT JOIN sessions s ON s.computer_id = c.id AND s.status = 'active'
LEFT JOIN members m ON m.id = s.member_id
";

fn computer_input(body: &Value) -> ApiResult<(String, String, i64)> {
    Ok((
        text(body, "name", "Computer name", true, 60)?,
        text(body, "location", "Location", false, 120)?,
        i64::from(boolean(body, "active", true)),
    ))
}

fn fetch_computer(conn: &Connection, id: i64) -> ApiResult<Value> {
    row(conn, &format!("{COMPUTER_SELECT} WHERE c.id = ?"), &[i(id)])?.ok_or_else(|| ApiError::new(404, "Computer not found."))
}

fn list_computers(ctx: &Ctx) -> ApiResult<Value> {
    Ok(json!({"items": rows(&ctx.conn()?, &format!("{COMPUTER_SELECT} ORDER BY c.name COLLATE NOCASE"), &[])?}))
}

fn create_computer(ctx: &Ctx) -> ApiResult<Value> {
    let (name, location, active) = computer_input(&ctx.body)?;
    let conn = ctx.conn()?;
    conn.execute("INSERT INTO computers (name, location, active) VALUES (?1, ?2, ?3)", params![name, location, active])?;
    fetch_computer(&conn, conn.last_insert_rowid())
}

fn update_computer(ctx: &Ctx, id: i64) -> ApiResult<Value> {
    let (name, location, active) = computer_input(&ctx.body)?;
    let conn = ctx.conn()?;
    if !fetch_computer(&conn, id)?["session_id"].is_null() && active == 0 {
        return Err(ApiError::new(409, "This computer is in use. Check the member out before disabling it."));
    }
    conn.execute(
        "UPDATE computers SET name = ?1, location = ?2, active = ?3 WHERE id = ?4",
        params![name, location, active, id],
    )?;
    fetch_computer(&conn, id)
}

fn delete_computer(ctx: &Ctx, id: i64) -> ApiResult<Value> {
    let conn = ctx.conn()?;
    if !fetch_computer(&conn, id)?["session_id"].is_null() {
        return Err(ApiError::new(409, "This computer is in use. Check the member out before deleting it."));
    }
    conn.execute("DELETE FROM computers WHERE id = ?1", params![id])?;
    Ok(json!({"ok": true}))
}

// ---------------------------------------------------------------- rate card

struct RateInput {
    name: String,
    description: String,
    rate_per_hour: f64,
    minimum_minutes: i64,
    increment_minutes: i64,
    max_charge: f64,
    active: bool,
    is_default: bool,
}

fn rate_input(body: &Value) -> ApiResult<RateInput> {
    Ok(RateInput {
        name: text(body, "name", "Plan name", true, 60)?,
        description: text(body, "description", "Description", false, 200)?,
        rate_per_hour: number(body, "rate_per_hour", "Rate per hour", 0.0, 1e7, None)?,
        minimum_minutes: integer(body, "minimum_minutes", "Minimum minutes", 0.0, 1440.0, Some(0.0))?,
        increment_minutes: integer(body, "increment_minutes", "Billing increment", 1.0, 1440.0, Some(1.0))?,
        max_charge: number(body, "max_charge", "Maximum charge", 0.0, 1e9, Some(0.0))?,
        active: boolean(body, "active", true),
        is_default: boolean(body, "is_default", false),
    })
}

fn fetch_rate(conn: &Connection, id: i64) -> ApiResult<Value> {
    row(conn, "SELECT * FROM rate_plans WHERE id = ?", &[i(id)])?.ok_or_else(|| ApiError::new(404, "Rate plan not found."))
}

fn normalise_default(conn: &Connection, preferred: Option<i64>) -> ApiResult<()> {
    if let Some(id) = preferred {
        conn.execute("UPDATE rate_plans SET is_default = (id = ?1)", params![id])?;
    }
    conn.execute("UPDATE rate_plans SET is_default = 0 WHERE active = 0", [])?;
    if one_i64(conn, "SELECT EXISTS(SELECT 1 FROM rate_plans WHERE is_default = 1)", &[])? == 0 {
        conn.execute(
            "UPDATE rate_plans SET is_default = 1 WHERE id = (SELECT id FROM rate_plans WHERE active = 1 ORDER BY id LIMIT 1)",
            [],
        )?;
    }
    Ok(())
}

fn list_rates(ctx: &Ctx) -> ApiResult<Value> {
    Ok(json!({"items": rows(&ctx.conn()?,
        "SELECT * FROM rate_plans ORDER BY is_default DESC, active DESC, name COLLATE NOCASE", &[])?}))
}

fn create_rate(ctx: &Ctx) -> ApiResult<Value> {
    let r = rate_input(&ctx.body)?;
    let mut conn = ctx.conn()?;
    let tx = conn.transaction()?;
    tx.execute(
        "INSERT INTO rate_plans (name, description, rate_per_hour, minimum_minutes, increment_minutes, max_charge, \
         active, is_default) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0)",
        params![r.name, r.description, r.rate_per_hour, r.minimum_minutes, r.increment_minutes, r.max_charge, i64::from(r.active)],
    )?;
    let id = tx.last_insert_rowid();
    normalise_default(&tx, (r.is_default && r.active).then_some(id))?;
    let result = fetch_rate(&tx, id)?;
    tx.commit()?;
    Ok(result)
}

fn update_rate(ctx: &Ctx, id: i64) -> ApiResult<Value> {
    let r = rate_input(&ctx.body)?;
    let mut conn = ctx.conn()?;
    let tx = conn.transaction()?;
    fetch_rate(&tx, id)?;
    tx.execute(
        "UPDATE rate_plans SET name = ?1, description = ?2, rate_per_hour = ?3, minimum_minutes = ?4, \
         increment_minutes = ?5, max_charge = ?6, active = ?7 WHERE id = ?8",
        params![r.name, r.description, r.rate_per_hour, r.minimum_minutes, r.increment_minutes, r.max_charge, i64::from(r.active), id],
    )?;
    normalise_default(&tx, (r.is_default && r.active).then_some(id))?;
    let result = fetch_rate(&tx, id)?;
    tx.commit()?;
    Ok(result)
}

fn delete_rate(ctx: &Ctx, id: i64) -> ApiResult<Value> {
    let mut conn = ctx.conn()?;
    let tx = conn.transaction()?;
    fetch_rate(&tx, id)?;
    tx.execute("DELETE FROM rate_plans WHERE id = ?1", params![id])?;
    normalise_default(&tx, None)?;
    tx.commit()?;
    Ok(json!({"ok": true}))
}

// ---------------------------------------------------------------- billing

pub fn compute_bill(session: &Value, end: DateTime<Utc>, discount: f64) -> ApiResult<Value> {
    let start = session["check_in"]
        .as_str()
        .and_then(parse_iso)
        .ok_or_else(|| ApiError::new(500, "Session has an invalid check-in time."))?;
    let elapsed = (end - start).num_seconds().max(0);
    let used = (elapsed + 59) / 60;
    let step = as_i64(&session["increment_minutes"]).max(1);
    let billable = (((used + step - 1) / step) * step).max(as_i64(&session["minimum_minutes"]));
    let mut subtotal = money(billable as f64 * as_f64(&session["rate_per_hour"]) / 60.0);
    let max_charge = as_f64(&session["max_charge"]);
    if max_charge > 0.0 {
        subtotal = subtotal.min(money(max_charge));
    }
    let discount = money(discount.max(0.0).min(subtotal));
    let tax = money((subtotal - discount) * as_f64(&session["tax_percent"]) / 100.0);
    Ok(json!({
        "check_out": iso(end),
        "elapsed_seconds": elapsed,
        "used_minutes": used,
        "billable_minutes": billable,
        "subtotal": subtotal,
        "discount": discount,
        "tax": tax,
        "total": money(subtotal - discount + tax),
    }))
}

// ---------------------------------------------------------------- sessions

const SESSION_SELECT: &str = "
SELECT s.*, m.first_name, m.last_name, m.email, m.phone, m.id_number
FROM sessions s JOIN members m ON m.id = s.member_id
";

fn fetch_session(conn: &Connection, id: i64) -> ApiResult<Value> {
    row(conn, &format!("{SESSION_SELECT} WHERE s.id = ?"), &[i(id)])?.ok_or_else(|| ApiError::new(404, "Session not found."))
}

fn active_session(conn: &Connection, id: i64) -> ApiResult<Value> {
    let session = fetch_session(conn, id)?;
    if session["status"] != "active" {
        return Err(ApiError::new(409, "This session is already closed."));
    }
    Ok(session)
}

fn session_filters(ctx: &Ctx) -> ApiResult<(String, Vec<Sql>)> {
    let mut clauses = Vec::new();
    let mut args = Vec::new();
    let status = ctx.q("status");
    if ["active", "completed", "cancelled"].contains(&status.as_str()) {
        clauses.push("s.status = ?".to_string());
        args.push(t(status));
    }
    if let Some(start) = query_date(&ctx.query, "from")? {
        clauses.push("s.check_in >= ?".to_string());
        args.push(t(iso(start)));
    }
    if let Some(end) = query_date(&ctx.query, "to")? {
        clauses.push("s.check_in < ?".to_string());
        args.push(t(iso(end)));
    }
    if let Ok(member_id) = ctx.q("member_id").parse::<i64>() {
        clauses.push("s.member_id = ?".to_string());
        args.push(i(member_id));
    }
    let term = ctx.q("q");
    if !term.is_empty() {
        let (clause, extra) = search_clause(
            &["m.first_name", "m.last_name", "m.first_name || ' ' || m.last_name", "m.email", "m.phone",
              "m.id_number", "s.computer_name", "s.plan_name"],
            &term,
        );
        clauses.push(clause);
        args.extend(extra);
    }
    let sql = if clauses.is_empty() { String::new() } else { format!(" WHERE {}", clauses.join(" AND ")) };
    Ok((sql, args))
}

fn list_sessions(ctx: &Ctx) -> ApiResult<Value> {
    let (filter, mut args) = session_filters(ctx)?;
    args.push(i(query_limit(&ctx.query, 1000, 10000)));
    let items = rows(&ctx.conn()?, &format!("{SESSION_SELECT}{filter} ORDER BY s.check_in DESC LIMIT ?"), &args)?;
    Ok(json!({"items": items, "server_now": Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)}))
}

fn check_in(ctx: &Ctx) -> ApiResult<Value> {
    let body = &ctx.body;
    let member_id = integer(body, "member_id", "Member", 1.0, 4.6e18, None)?;
    let computer_id = optional_id(body, "computer_id", "Computer")?;
    let rate_plan_id = optional_id(body, "rate_plan_id", "Rate plan")?;
    let notes = text(body, "notes", "Notes", false, 500)?;
    let mut conn = ctx.conn()?;
    let tx = conn.transaction()?;

    let member = fetch_member(&tx, member_id)?;
    if !member["active_session_id"].is_null() {
        return Err(ApiError::new(409, format!(
            "{} {} is already checked in.", member["first_name"].as_str().unwrap_or(""), member["last_name"].as_str().unwrap_or(""))));
    }

    let mut computer_name = String::new();
    if let Some(id) = computer_id {
        let computer = fetch_computer(&tx, id)?;
        let name = computer["name"].as_str().unwrap_or_default().to_string();
        if as_i64(&computer["active"]) == 0 {
            return Err(ApiError::new(409, format!("{name} is disabled.")));
        }
        if !computer["session_id"].is_null() {
            return Err(ApiError::new(409, format!(
                "{name} is already in use by {}.", computer["session_member"].as_str().unwrap_or("someone"))));
        }
        computer_name = name;
    }

    let plan = match rate_plan_id {
        None => row(&tx, "SELECT * FROM rate_plans WHERE is_default = 1 AND active = 1", &[])?
            .ok_or_else(|| ApiError::new(409, "No default rate plan is set. Update the rate card first."))?,
        Some(id) => {
            let plan = fetch_rate(&tx, id)?;
            if as_i64(&plan["active"]) == 0 {
                return Err(ApiError::new(409, format!("Rate plan '{}' is inactive.", plan["name"].as_str().unwrap_or(""))));
            }
            plan
        }
    };

    let tax = tax_percent(&load_settings(&tx)?);
    tx.execute(
        "INSERT INTO sessions (member_id, computer_id, computer_name, rate_plan_id, plan_name, rate_per_hour, \
         minimum_minutes, increment_minutes, max_charge, tax_percent, check_in, notes) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            member_id, computer_id, computer_name, as_i64(&plan["id"]), plan["name"].as_str().unwrap_or_default(),
            as_f64(&plan["rate_per_hour"]), as_i64(&plan["minimum_minutes"]), as_i64(&plan["increment_minutes"]),
            as_f64(&plan["max_charge"]), tax, iso(now_utc()), notes
        ],
    )?;
    let session = fetch_session(&tx, tx.last_insert_rowid())?;
    tx.commit()?;
    Ok(session)
}

fn quote(ctx: &Ctx, id: i64) -> ApiResult<Value> {
    let discount = number(&json!({"d": ctx.q("discount")}), "d", "Discount", 0.0, 1e9, Some(0.0))?;
    let session = active_session(&ctx.conn()?, id)?;
    let bill = compute_bill(&session, now_utc(), discount)?;
    Ok(json!({"session": session, "bill": bill}))
}

fn check_out(ctx: &Ctx, id: i64) -> ApiResult<Value> {
    let body = &ctx.body;
    let discount = number(body, "discount", "Discount", 0.0, 1e9, Some(0.0))?;
    let payment = text(body, "payment_method", "Payment method", true, 30)?;
    if !PAYMENT_METHODS.contains(&payment.as_str()) {
        return Err(ApiError::new(400, "Choose a valid payment method."));
    }
    let conn = ctx.conn()?;
    let session = active_session(&conn, id)?;
    let notes = if body.get("notes").is_some() {
        text(body, "notes", "Notes", false, 500)?
    } else {
        session["notes"].as_str().unwrap_or_default().to_string()
    };
    let bill = compute_bill(&session, now_utc(), discount)?;
    let changed = conn.execute(
        "UPDATE sessions SET check_out = ?1, duration_minutes = ?2, billable_minutes = ?3, subtotal = ?4, \
         discount = ?5, tax = ?6, total = ?7, payment_method = ?8, notes = ?9, status = 'completed' \
         WHERE id = ?10 AND status = 'active'",
        params![
            bill["check_out"].as_str(), as_i64(&bill["used_minutes"]), as_i64(&bill["billable_minutes"]),
            as_f64(&bill["subtotal"]), as_f64(&bill["discount"]), as_f64(&bill["tax"]), as_f64(&bill["total"]),
            payment, notes, id
        ],
    )?;
    if changed == 0 {
        return Err(ApiError::new(409, "This session is already closed."));
    }
    fetch_session(&conn, id)
}

fn cancel_session(ctx: &Ctx, id: i64) -> ApiResult<Value> {
    let conn = ctx.conn()?;
    let session = active_session(&conn, id)?;
    let end = now_utc();
    let start = session["check_in"].as_str().and_then(parse_iso).unwrap_or(end);
    let used = ((end - start).num_seconds().max(0) + 59) / 60;
    conn.execute(
        "UPDATE sessions SET status = 'cancelled', check_out = ?1, duration_minutes = ?2, billable_minutes = 0, \
         subtotal = 0, discount = 0, tax = 0, total = 0 WHERE id = ?3 AND status = 'active'",
        params![iso(end), used, id],
    )?;
    fetch_session(&conn, id)
}

// ---------------------------------------------------------------- reports

#[derive(Default)]
struct Buckets {
    order: Vec<Value>,
    index: HashMap<String, usize>,
}

impl Buckets {
    fn add(&mut self, key: String, minutes: i64, revenue: f64, member_id: Option<i64>) {
        let idx = *self.index.entry(key.clone()).or_insert_with(|| {
            let mut entry = json!({"name": key, "sessions": 0, "minutes": 0, "revenue": 0.0});
            if let Some(id) = member_id {
                entry["member_id"] = json!(id);
            }
            self.order.push(entry);
            self.order.len() - 1
        });
        let entry = &mut self.order[idx];
        entry["sessions"] = json!(as_i64(&entry["sessions"]) + 1);
        entry["minutes"] = json!(as_i64(&entry["minutes"]) + minutes);
        entry["revenue"] = json!(as_f64(&entry["revenue"]) + revenue);
    }

    fn finish(mut self, by_name: bool, limit: Option<usize>) -> Vec<Value> {
        if by_name {
            self.order.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
        } else {
            self.order.sort_by(|a, b| as_f64(&b["revenue"]).total_cmp(&as_f64(&a["revenue"])));
        }
        for entry in &mut self.order {
            entry["revenue"] = json!(money(as_f64(&entry["revenue"])));
        }
        if let Some(n) = limit {
            self.order.truncate(n);
        }
        self.order
    }
}

fn reports(ctx: &Ctx) -> ApiResult<Value> {
    let start = query_date(&ctx.query, "from")?.unwrap_or_else(local_midnight_utc);
    let end = query_date(&ctx.query, "to")?.unwrap_or_else(|| now_utc() + ChronoDuration::days(1));
    let items = rows(
        &ctx.conn()?,
        &format!("{SESSION_SELECT} WHERE s.status = 'completed' AND s.check_out >= ? AND s.check_out < ? ORDER BY s.check_out"),
        &[t(iso(start)), t(iso(end))],
    )?;

    let (mut by_day, mut by_payment, mut by_plan, mut by_member) =
        (Buckets::default(), Buckets::default(), Buckets::default(), Buckets::default());
    let (mut minutes, mut billable, mut subtotal, mut discount, mut tax, mut revenue) = (0i64, 0i64, 0.0, 0.0, 0.0, 0.0);
    for s in &items {
        let used = as_i64(&s["duration_minutes"]);
        let total = as_f64(&s["total"]);
        minutes += used;
        billable += as_i64(&s["billable_minutes"]);
        subtotal += as_f64(&s["subtotal"]);
        discount += as_f64(&s["discount"]);
        tax += as_f64(&s["tax"]);
        revenue += total;
        let member = format!(
            "{} {} ({})",
            s["first_name"].as_str().unwrap_or(""), s["last_name"].as_str().unwrap_or(""), s["id_number"].as_str().unwrap_or("")
        );
        by_day.add(local_day(&s["check_out"]), used, total, None);
        by_payment.add(s["payment_method"].as_str().unwrap_or("Unknown").to_string(), used, total, None);
        by_plan.add(s["plan_name"].as_str().unwrap_or_default().to_string(), used, total, None);
        by_member.add(member, used, total, Some(as_i64(&s["member_id"])));
    }
    let count = items.len();
    let revenue = money(revenue);
    let (avg_minutes, avg_revenue) = if count > 0 {
        ((minutes as f64 / count as f64 * 10.0).round() / 10.0, money(revenue / count as f64))
    } else {
        (0.0, 0.0)
    };
    Ok(json!({
        "from": iso(start),
        "to": iso(end),
        "summary": {
            "sessions": count, "minutes": minutes, "billable_minutes": billable,
            "subtotal": money(subtotal), "discount": money(discount), "tax": money(tax), "revenue": revenue,
            "avg_minutes": avg_minutes, "avg_revenue": avg_revenue,
        },
        "by_day": by_day.finish(true, None),
        "by_payment": by_payment.finish(false, None),
        "by_plan": by_plan.finish(false, None),
        "top_members": by_member.finish(false, Some(10)),
    }))
}

// ---------------------------------------------------------------- files

fn csv_bytes(header: &[&str], data: Vec<Vec<String>>) -> ApiResult<Vec<u8>> {
    // Neutralise spreadsheet formula injection.
    let safe = |v: String| if v.starts_with(['=', '+', '-', '@', '\t', '\r']) { format!("'{v}") } else { v };
    let mut writer = csv::Writer::from_writer(Vec::from("\u{feff}".as_bytes()));
    let fail = |e: csv::Error| ApiError::new(500, format!("Could not create the CSV file: {e}"));
    writer.write_record(header).map_err(fail)?;
    for record in data {
        writer.write_record(record.into_iter().map(safe)).map_err(fail)?;
    }
    writer.into_inner().map_err(|e| ApiError::new(500, format!("Could not create the CSV file: {e}")))
}

fn cell(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

fn save_dialog(ctx: &Ctx, file_name: &str, filter: &str, extension: &str) -> ApiResult<Option<PathBuf>> {
    Ok(ctx.app()?
        .dialog()
        .file()
        .set_file_name(file_name)
        .add_filter(filter, &[extension])
        .blocking_save_file()
        .and_then(|p| p.into_path().ok()))
}

fn saved(path: &std::path::Path) -> Value {
    json!({"saved": path.display().to_string()})
}

fn export_csv(ctx: &Ctx, kind: &str) -> ApiResult<Value> {
    let conn = ctx.conn()?;
    let today = chrono::Local::now().format("%Y%m%d");
    let (file_name, bytes) = match kind {
        "members" => {
            let items = rows(&conn, &format!("{MEMBER_SELECT} ORDER BY m.first_name COLLATE NOCASE, m.last_name COLLATE NOCASE"), &[])?;
            let data = items.iter().map(|m| vec![
                cell(&m["id"]), cell(&m["first_name"]), cell(&m["last_name"]), cell(&m["email"]), cell(&m["phone"]),
                cell(&m["id_number"]), cell(&m["visits"]), money(as_f64(&m["total_spent"])).to_string(),
                local_text(&m["last_visit"]), local_text(&m["created_at"]), cell(&m["notes"]),
            ]).collect();
            (format!("members-{today}.csv"), csv_bytes(
                &["Member #", "First name", "Last name", "Email", "Phone", "ID number", "Visits", "Total spent",
                  "Last visit", "Registered", "Notes"], data)?)
        }
        "sessions" => {
            let (filter, args) = session_filters(ctx)?;
            let items = rows(&conn, &format!("{SESSION_SELECT}{filter} ORDER BY s.check_in DESC"), &args)?;
            let data = items.iter().map(|s| vec![
                cell(&s["id"]), cell(&s["first_name"]), cell(&s["last_name"]), cell(&s["id_number"]), cell(&s["phone"]),
                cell(&s["email"]), cell(&s["computer_name"]), cell(&s["plan_name"]), cell(&s["rate_per_hour"]),
                local_text(&s["check_in"]), local_text(&s["check_out"]), cell(&s["duration_minutes"]),
                cell(&s["billable_minutes"]), cell(&s["subtotal"]), cell(&s["discount"]), cell(&s["tax"]),
                cell(&s["total"]), cell(&s["payment_method"]), cell(&s["status"]), cell(&s["notes"]),
            ]).collect();
            (format!("sessions-{today}.csv"), csv_bytes(
                &["Receipt #", "First name", "Last name", "ID number", "Phone", "Email", "Computer", "Plan",
                  "Rate per hour", "Check in", "Check out", "Minutes used", "Minutes billed", "Subtotal", "Discount",
                  "Tax", "Total", "Payment", "Status", "Notes"], data)?)
        }
        _ => return Err(ApiError::new(404, "Not found.")),
    };
    let Some(path) = save_dialog(ctx, &file_name, "CSV spreadsheet", "csv")? else {
        return Ok(json!({"cancelled": true}));
    };
    std::fs::write(&path, bytes)?;
    Ok(saved(&path))
}

fn save_backup(ctx: &Ctx) -> ApiResult<Value> {
    let name = format!("library-backup-{}.db", chrono::Local::now().format("%Y%m%d-%H%M"));
    let Some(path) = save_dialog(ctx, &name, "Digital Library backup", "db")? else {
        return Ok(json!({"cancelled": true}));
    };
    // SQLite's backup API writes into an existing database file, so start from a clean file.
    let _ = std::fs::remove_file(&path);
    ctx.conn()?.backup(DatabaseName::Main, &path, None)?;
    Ok(saved(&path))
}

fn restore_backup(ctx: &Ctx) -> ApiResult<Value> {
    let Some(source) = ctx.app()?
        .dialog()
        .file()
        .add_filter("Digital Library backup", &["db"])
        .blocking_pick_file()
        .and_then(|p| p.into_path().ok())
    else {
        return Ok(json!({"cancelled": true}));
    };
    let result = restore_from(ctx.state, &source)?;
    ctx.state.sign_out();
    Ok(result)
}

pub fn restore_from(state: &AppState, source: &std::path::Path) -> ApiResult<Value> {
    let invalid = || ApiError::new(400, "This is not a Digital Library backup file.");
    let data = std::fs::read(source)?;
    if !data.starts_with(SQLITE_MAGIC) {
        return Err(invalid());
    }
    let mut nonce = [0u8; 8];
    getrandom::getrandom(&mut nonce).map_err(|_| ApiError::new(500, "Random number generator unavailable."))?;
    let temp = std::env::temp_dir().join(format!("digital-library-restore-{}.db", hex::encode(nonce)));
    std::fs::write(&temp, &data)?;

    let outcome = (|| -> ApiResult<PathBuf> {
        {
            let check = Connection::open(&temp).map_err(|_| invalid())?;
            let tables: Vec<String> = rows(&check, "SELECT name FROM sqlite_master WHERE type = 'table'", &[])
                .map_err(|_| invalid())?
                .iter()
                .filter_map(|r| r["name"].as_str().map(str::to_string))
                .collect();
            if !REQUIRED_TABLES.iter().all(|name| tables.iter().any(|t| t == name)) {
                return Err(invalid());
            }
            let integrity: String = check.query_row("PRAGMA integrity_check", [], |r| r.get(0)).map_err(|_| invalid())?;
            if integrity != "ok" {
                return Err(ApiError::new(400, "The backup file is damaged."));
            }
        }
        let safety = state.data_dir.join(format!(
            "library-before-restore-{}.db", chrono::Local::now().format("%Y%m%d-%H%M%S")));
        let mut current = db::connect(&state.db_path)?;
        current.backup(DatabaseName::Main, &safety, None)?;
        current.restore(DatabaseName::Main, &temp, None::<fn(Progress)>)?;
        Ok(safety)
    })();
    let _ = std::fs::remove_file(&temp);
    let safety = outcome?;

    db::init(&state.db_path)?;
    let conn = db::connect(&state.db_path)?;
    Ok(json!({
        "ok": true,
        "members": one_i64(&conn, "SELECT COUNT(*) FROM members", &[])?,
        "sessions": one_i64(&conn, "SELECT COUNT(*) FROM sessions", &[])?,
        "previous_data": safety.display().to_string(),
        "signed_out": true,
    }))
}

fn print_window(ctx: &Ctx) -> ApiResult<Value> {
    let window = ctx.app()?.get_webview_window("main").ok_or_else(|| ApiError::new(500, "App window not found."))?;
    window.print().map_err(|e| ApiError::new(500, format!("Could not print: {e}")))?;
    Ok(json!({"ok": true}))
}

// ---------------------------------------------------------------- tests

#[cfg(test)]
mod tests {
    use super::*;

    fn test_state() -> (AppState, PathBuf) {
        let mut nonce = [0u8; 8];
        getrandom::getrandom(&mut nonce).unwrap();
        let dir = std::env::temp_dir().join(format!("dl-test-{}", hex::encode(nonce)));
        let state = AppState::new(dir.clone());
        db::init(&state.db_path).unwrap();
        (state, dir)
    }

    fn call(state: &AppState, method: &str, path: &str, body: Value) -> ApiResult<Value> {
        dispatch(None, state, method, path, body)
    }

    #[test]
    fn billing_matches_python_version() {
        let mut s = json!({"check_in": "2026-10-27T10:00:00Z", "increment_minutes": 15, "minimum_minutes": 30,
                           "rate_per_hour": 60.0, "max_charge": 0.0, "tax_percent": 18.0});
        let start = parse_iso("2026-10-27T10:00:00Z").unwrap();
        let cases = [(10, 30, 30.0, 5.4, 35.4), (47, 60, 60.0, 10.8, 70.8), (91, 105, 105.0, 18.9, 123.9), (180, 180, 180.0, 32.4, 212.4)];
        for (minutes, billable, subtotal, tax, total) in cases {
            let bill = compute_bill(&s, start + ChronoDuration::minutes(minutes), 0.0).unwrap();
            assert_eq!(bill["billable_minutes"], json!(billable));
            assert_eq!(bill["subtotal"], json!(subtotal));
            assert_eq!(bill["tax"], json!(tax));
            assert_eq!(bill["total"], json!(total));
        }
        s["max_charge"] = json!(100.0);
        let capped = compute_bill(&s, start + ChronoDuration::minutes(180), 0.0).unwrap();
        assert_eq!(capped["total"], json!(118.0));
        s["tax_percent"] = json!(0.0);
        let discounted = compute_bill(&s, start + ChronoDuration::minutes(1), 5.0).unwrap();
        assert_eq!(discounted["total"], json!(25.0));
    }

    #[test]
    fn money_rounds_half_up() {
        assert_eq!(money(1.005), 1.01);
        assert_eq!(money(2.675), 2.68);
        assert_eq!(money(0.1 + 0.2), 0.3);
    }

    #[test]
    fn verifies_password_hash_from_python_version() {
        let python_hash = "pbkdf2_sha256$600000$abc0f354fa15c26733d4d0256bf4bedf$94efb08337d89a9cce0cf9495b21a163fa9f7d9f464f5b2d6ace869a5d7697bf";
        assert!(verify_password("Admin@123", python_hash));
        assert!(!verify_password("admin@123", python_hash));
        assert!(verify_password("Secret#99", &hash_password("Secret#99")));
        assert!(!verify_password("x", "garbage"));
    }

    #[test]
    fn full_desk_workflow() {
        let (state, dir) = test_state();
        assert_eq!(call(&state, "GET", "/api/members", json!({})).unwrap_err().status, 401);
        assert_eq!(call(&state, "POST", "/api/auth/login", json!({"username": "admin", "password": "nope"})).unwrap_err().status, 401);
        let status = call(&state, "POST", "/api/auth/login", json!({"username": "ADMIN", "password": "Admin@123"})).unwrap();
        assert_eq!(status["default_password"], json!(true));

        let member = call(&state, "POST", "/api/members", json!({
            "first_name": "Asha", "last_name": "Rao", "email": "Asha@Ex.com", "phone": "+91 98765 43210", "id_number": "LIB-001"})).unwrap();
        assert_eq!(member["email"], json!("asha@ex.com"));
        let dup = call(&state, "POST", "/api/members", json!({
            "first_name": "X", "last_name": "Y", "email": "x@y.co", "phone": "1234567", "id_number": "lib-001"})).unwrap_err();
        assert_eq!((dup.status, dup.error.as_str()), (409, "A member with this ID number already exists."));
        assert_eq!(call(&state, "POST", "/api/members", json!({
            "first_name": "X", "last_name": "Y", "email": "bad", "phone": "1234567", "id_number": "Z"})).unwrap_err().status, 400);

        let found = call(&state, "GET", "/api/members?q=asha%20r", json!({})).unwrap();
        assert_eq!(found["items"].as_array().unwrap().len(), 1);

        let computers = call(&state, "GET", "/api/computers", json!({})).unwrap();
        let pc = computers["items"][0]["id"].clone();
        let session = call(&state, "POST", "/api/sessions/checkin", json!({"member_id": member["id"], "computer_id": pc})).unwrap();
        assert_eq!(session["plan_name"], json!("Standard"));
        assert_eq!(call(&state, "POST", "/api/sessions/checkin", json!({"member_id": member["id"]})).unwrap_err().status, 409);

        let id = session["id"].as_i64().unwrap();
        let quote = call(&state, "GET", &format!("/api/sessions/{id}/quote?discount=5"), json!({})).unwrap();
        assert_eq!(quote["bill"]["total"], json!(25.0));
        let done = call(&state, "POST", &format!("/api/sessions/{id}/checkout"), json!({"discount": "5", "payment_method": "UPI"})).unwrap();
        assert_eq!((done["status"].clone(), done["total"].clone()), (json!("completed"), json!(25.0)));
        assert_eq!(call(&state, "POST", &format!("/api/sessions/{id}/checkout"), json!({"payment_method": "UPI"})).unwrap_err().status, 409);

        let report = call(&state, "GET", "/api/reports", json!({})).unwrap();
        assert_eq!(report["summary"]["revenue"], json!(25.0));
        assert_eq!(report["by_payment"][0]["name"], json!("UPI"));
        let dashboard = call(&state, "GET", "/api/dashboard", json!({})).unwrap();
        assert_eq!(dashboard["today_revenue"], json!(25.0));
        assert_eq!(call(&state, "DELETE", &format!("/api/members/{}", member["id"]), json!({})).unwrap_err().status, 409);

        let rates = call(&state, "GET", "/api/rates", json!({})).unwrap();
        let mut student = rates["items"].as_array().unwrap().iter().find(|r| r["name"] == "Student").unwrap().clone();
        student["is_default"] = json!(true);
        call(&state, "PUT", &format!("/api/rates/{}", student["id"]), student.clone()).unwrap();
        let rates = call(&state, "GET", "/api/rates", json!({})).unwrap();
        assert_eq!(rates["items"][0]["name"], json!("Student"));
        assert_eq!(rates["items"].as_array().unwrap().iter().filter(|r| r["is_default"] == json!(1)).count(), 1);

        // Password change via current password, then reset via default password while signed out.
        assert_eq!(call(&state, "POST", "/api/auth/change-password", json!({"current_password": "Admin@123", "new_password": "short"})).unwrap_err().status, 400);
        call(&state, "POST", "/api/auth/change-password", json!({"current_password": "Admin@123", "new_password": "Library#2026"})).unwrap();
        call(&state, "POST", "/api/auth/logout", json!({})).unwrap();
        assert_eq!(call(&state, "POST", "/api/auth/login", json!({"username": "admin", "password": "Admin@123"})).unwrap_err().status, 401);
        call(&state, "POST", "/api/auth/change-password", json!({"username": "admin", "current_password": "Admin@123", "new_password": "Fresh#2026"})).unwrap();
        let status = call(&state, "POST", "/api/auth/login", json!({"username": "admin", "password": "Fresh#2026"})).unwrap();
        assert_eq!(status["default_password"], json!(false));

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn lockout_after_failed_logins() {
        let (state, dir) = test_state();
        for n in 0..5 {
            let _ = call(&state, "POST", "/api/auth/login", json!({"username": "admin", "password": format!("bad{n}")}));
        }
        let err = call(&state, "POST", "/api/auth/login", json!({"username": "admin", "password": "Admin@123"})).unwrap_err();
        assert_eq!(err.status, 429);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn restore_imports_data_and_credentials() {
        let (source, source_dir) = test_state();
        call(&source, "POST", "/api/auth/login", json!({"username": "admin", "password": "Admin@123"})).unwrap();
        call(&source, "POST", "/api/members", json!({
            "first_name": "Ravi", "last_name": "K", "email": "r@k.com", "phone": "9999988888", "id_number": "A-1"})).unwrap();
        call(&source, "POST", "/api/auth/change-password", json!({"current_password": "Admin@123", "new_password": "Export#2026"})).unwrap();
        let backup = source_dir.join("backup.db");
        db::connect(&source.db_path).unwrap().backup(DatabaseName::Main, &backup, None).unwrap();

        let (target, target_dir) = test_state();
        let result = restore_from(&target, &backup).unwrap();
        assert_eq!(result["members"], json!(1));
        assert_eq!(call(&target, "POST", "/api/auth/login", json!({"username": "admin", "password": "Admin@123"})).unwrap_err().status, 401);
        call(&target, "POST", "/api/auth/login", json!({"username": "admin", "password": "Export#2026"})).unwrap();

        let junk = target_dir.join("junk.db");
        std::fs::write(&junk, b"hello").unwrap();
        assert_eq!(restore_from(&target, &junk).unwrap_err().status, 400);

        let _ = std::fs::remove_dir_all(source_dir);
        let _ = std::fs::remove_dir_all(target_dir);
    }
}
