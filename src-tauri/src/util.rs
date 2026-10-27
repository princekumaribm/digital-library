use crate::error::{ApiError, ApiResult};
use chrono::{DateTime, Local, NaiveDateTime, TimeZone, Timelike, Utc};
use pbkdf2::pbkdf2_hmac;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::{Decimal, RoundingStrategy};
use serde_json::Value;
use sha2::Sha256;
use std::collections::HashMap;
use std::str::FromStr;

pub const DEFAULT_USERNAME: &str = "admin";
pub const DEFAULT_PASSWORD: &str = "Admin@123";
const PBKDF2_ITERATIONS: u32 = 600_000;

// ------------------------------------------------------------ time

pub fn now_utc() -> DateTime<Utc> {
    Utc::now().with_nanosecond(0).unwrap_or_else(Utc::now)
}

pub fn iso(dt: DateTime<Utc>) -> String {
    dt.format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

pub fn parse_iso(value: &str) -> Option<DateTime<Utc>> {
    let value = value.trim();
    if let Ok(dt) = DateTime::parse_from_rfc3339(value) {
        return Some(dt.with_timezone(&Utc));
    }
    NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S%.f")
        .ok()
        .and_then(|naive| Local.from_local_datetime(&naive).earliest())
        .map(|dt| dt.with_timezone(&Utc))
}

pub fn local_midnight_utc() -> DateTime<Utc> {
    let midnight = Local::now().date_naive().and_hms_opt(0, 0, 0).expect("valid midnight");
    Local
        .from_local_datetime(&midnight)
        .earliest()
        .unwrap_or_else(Local::now)
        .with_timezone(&Utc)
}

pub fn local_text(value: &Value) -> String {
    value
        .as_str()
        .and_then(parse_iso)
        .map(|dt| dt.with_timezone(&Local).format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_default()
}

pub fn local_day(value: &Value) -> String {
    value
        .as_str()
        .and_then(parse_iso)
        .map(|dt| dt.with_timezone(&Local).format("%Y-%m-%d").to_string())
        .unwrap_or_default()
}

// ------------------------------------------------------------ numbers

/// Rounds to 2 decimals, half away from zero, on the shortest decimal form (matches Python's Decimal(str(x))).
pub fn money(value: f64) -> f64 {
    if !value.is_finite() {
        return 0.0;
    }
    Decimal::from_str(&value.to_string())
        .ok()
        .and_then(|d| d.round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero).to_f64())
        .unwrap_or(0.0)
}

pub fn as_f64(value: &Value) -> f64 {
    value.as_f64().unwrap_or(0.0)
}

pub fn as_i64(value: &Value) -> i64 {
    value.as_i64().or_else(|| value.as_f64().map(|f| f as i64)).unwrap_or(0)
}

fn fmt_num(n: f64) -> String {
    if n.fract() == 0.0 && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

// ------------------------------------------------------------ input validation

pub fn text(body: &Value, key: &str, label: &str, required: bool, max_len: usize) -> ApiResult<String> {
    let value = match body.get(key) {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(s)) => s.trim().to_string(),
        Some(Value::Number(n)) => n.to_string(),
        Some(_) => return Err(ApiError::new(400, format!("{label} is invalid."))),
    };
    if required && value.is_empty() {
        return Err(ApiError::new(400, format!("{label} is required.")));
    }
    if value.chars().count() > max_len {
        return Err(ApiError::new(400, format!("{label} must be at most {max_len} characters.")));
    }
    Ok(value)
}

pub fn number(body: &Value, key: &str, label: &str, min: f64, max: f64, default: Option<f64>) -> ApiResult<f64> {
    let required = || ApiError::new(400, format!("{label} is required."));
    let not_number = || ApiError::new(400, format!("{label} must be a number."));
    let num = match body.get(key) {
        None | Some(Value::Null) => default.ok_or_else(required)?,
        Some(Value::String(s)) if s.trim().is_empty() => default.ok_or_else(required)?,
        Some(Value::String(s)) => s.trim().parse::<f64>().map_err(|_| not_number())?,
        Some(Value::Number(n)) => n.as_f64().ok_or_else(not_number)?,
        Some(Value::Bool(b)) => f64::from(u8::from(*b)),
        Some(_) => return Err(not_number()),
    };
    if !num.is_finite() || num < min || num > max {
        return Err(ApiError::new(
            400,
            format!("{label} must be between {} and {}.", fmt_num(min), fmt_num(max)),
        ));
    }
    Ok(num)
}

pub fn integer(body: &Value, key: &str, label: &str, min: f64, max: f64, default: Option<f64>) -> ApiResult<i64> {
    let num = number(body, key, label, min, max, default)?;
    if num.fract() != 0.0 {
        return Err(ApiError::new(400, format!("{label} must be a whole number.")));
    }
    Ok(num as i64)
}

pub fn optional_id(body: &Value, key: &str, label: &str) -> ApiResult<Option<i64>> {
    match body.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) if s.trim().is_empty() => Ok(None),
        Some(_) => integer(body, key, label, 1.0, 4.6e18, None).map(Some),
    }
}

pub fn boolean(body: &Value, key: &str, default: bool) -> bool {
    match body.get(key) {
        None | Some(Value::Null) => default,
        Some(Value::Bool(b)) => *b,
        Some(Value::String(s)) => matches!(s.trim().to_lowercase().as_str(), "1" | "true" | "yes" | "on"),
        Some(Value::Number(n)) => n.as_f64().is_some_and(|f| f != 0.0),
        Some(_) => true,
    }
}

pub fn query_param(query: &HashMap<String, String>, key: &str) -> String {
    query.get(key).map(|v| v.trim().to_string()).unwrap_or_default()
}

pub fn query_date(query: &HashMap<String, String>, key: &str) -> ApiResult<Option<DateTime<Utc>>> {
    let value = query_param(query, key);
    if value.is_empty() {
        return Ok(None);
    }
    parse_iso(&value)
        .map(Some)
        .ok_or_else(|| ApiError::new(400, format!("Invalid date for '{key}'.")))
}

pub fn query_limit(query: &HashMap<String, String>, default: i64, max: i64) -> i64 {
    query_param(query, "limit").parse::<i64>().map(|n| n.clamp(1, max)).unwrap_or(default)
}

// ------------------------------------------------------------ passwords

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

pub fn hash_password(password: &str) -> String {
    let mut salt = [0u8; 16];
    getrandom::getrandom(&mut salt).expect("operating system random number generator");
    let mut digest = [0u8; 32];
    pbkdf2_hmac::<Sha256>(password.as_bytes(), &salt, PBKDF2_ITERATIONS, &mut digest);
    format!("pbkdf2_sha256${PBKDF2_ITERATIONS}${}${}", hex::encode(salt), hex::encode(digest))
}

pub fn verify_password(password: &str, stored: &str) -> bool {
    let parts: Vec<&str> = stored.split('$').collect();
    if parts.len() != 4 || parts[0] != "pbkdf2_sha256" {
        return false;
    }
    // Bound iterations so a tampered backup can't make sign-in hang.
    let Ok(iterations) = parts[1].parse::<u32>() else { return false };
    if !(1..=5_000_000).contains(&iterations) {
        return false;
    }
    let (Ok(salt), Ok(expected)) = (hex::decode(parts[2]), hex::decode(parts[3])) else { return false };
    if expected.is_empty() || expected.len() > 64 {
        return false;
    }
    let mut digest = vec![0u8; expected.len()];
    pbkdf2_hmac::<Sha256>(password.as_bytes(), &salt, iterations, &mut digest);
    constant_time_eq(&digest, &expected)
}

pub fn is_default_password(password: &str) -> bool {
    constant_time_eq(password.as_bytes(), DEFAULT_PASSWORD.as_bytes())
}

pub fn password(body: &Value, key: &str, label: &str) -> ApiResult<String> {
    match body.get(key) {
        Some(Value::String(s)) if !s.is_empty() => {
            if s.chars().count() > 128 {
                Err(ApiError::new(400, format!("{label} must be at most 128 characters.")))
            } else {
                Ok(s.clone())
            }
        }
        _ => Err(ApiError::new(400, format!("{label} is required."))),
    }
}

pub fn check_new_password(password: &str) -> ApiResult<()> {
    let ok = password.chars().count() >= 8
        && password.chars().any(|c| c.is_ascii_alphabetic())
        && password.chars().any(|c| c.is_ascii_digit());
    if ok {
        Ok(())
    } else {
        Err(ApiError::new(400, "New password must be at least 8 characters and include a letter and a number."))
    }
}
