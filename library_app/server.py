#!/usr/bin/env python3
"""Digital Library: local member check-in/check-out and billing for library computers."""

from __future__ import annotations

import csv
import hashlib
import hmac
import io
import json
import math
import mimetypes
import os
import re
import secrets
import sqlite3
import sys
import tempfile
import threading
import time
import traceback
import webbrowser
from contextlib import closing, contextmanager
from datetime import datetime, timedelta, timezone
from decimal import ROUND_HALF_UP, Decimal
from http.cookies import CookieError, SimpleCookie
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import parse_qs, unquote, urlparse
from urllib.request import urlopen

APP_ID = "digital-library"
VERSION = "1.0.0"
HOST = "127.0.0.1"
DEFAULT_PORT = int(os.environ.get("LIBRARY_PORT", "8765"))
PORT_RANGE = 20
# PyInstaller unpacks bundled files under sys._MEIPASS; resolve() follows the .app's symlinks.
STATIC_DIR = (Path(getattr(sys, "_MEIPASS", Path(__file__).parent)) / "static").resolve()
LOG_PATH = Path.home() / "Library" / "Logs" / "DigitalLibrary.log"
DATA_DIR = Path(
    os.environ.get("LIBRARY_DATA_DIR")
    or Path.home() / "Library" / "Application Support" / "DigitalLibrary"
)
DB_PATH = DATA_DIR / "library.db"
MAX_BODY = 1_000_000
MAX_UPLOAD = 1_000_000_000
SQLITE_MAGIC = b"SQLite format 3\x00"
REQUIRED_TABLES = {"settings", "members", "computers", "rate_plans", "sessions"}
PAYMENT_METHODS = ["Cash", "Card", "UPI", "Bank transfer", "Other"]
SETTINGS_DEFAULTS = {"library_name": "Digital Library", "currency": "₹", "tax_percent": "0"}
EMAIL_RE = re.compile(r"^[^@\s]+@[^@\s]+\.[^@\s]+$")
PHONE_RE = re.compile(r"^[0-9+()\-.\s]{6,25}$")
DEFAULT_USERNAME = "admin"
DEFAULT_PASSWORD = "Admin@123"
PBKDF2_ITERATIONS = 600_000
SESSION_COOKIE = "dl_session"
SESSION_TTL = 12 * 3600
MAX_FAILED_LOGINS = 5
LOCKOUT_SECONDS = 60

SCHEMA = """
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
"""


class ApiError(Exception):
    def __init__(self, status: int, message: str):
        super().__init__(message)
        self.status = status
        self.message = message


class Raw:
    def __init__(self, body: bytes, content_type: str, filename: str | None = None):
        self.body = body
        self.content_type = content_type
        self.filename = filename


class Request:
    def __init__(self, handler, match, query, body):
        self.handler = handler
        self.match = match
        self.query = query
        self.body = body

    def id(self) -> int:
        return int(self.match.group(1))


# ---------------------------------------------------------------- utilities

def now_utc() -> datetime:
    return datetime.now(timezone.utc).replace(microsecond=0)


def iso(dt: datetime) -> str:
    return dt.astimezone(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def parse_iso(value: str) -> datetime:
    value = value.strip()
    if value.endswith("Z"):
        value = value[:-1] + "+00:00"
    dt = datetime.fromisoformat(value)
    if dt.tzinfo is None:
        dt = dt.astimezone()
    return dt.astimezone(timezone.utc)


def local_text(value: str | None) -> str:
    return parse_iso(value).astimezone().strftime("%Y-%m-%d %H:%M") if value else ""


def local_midnight_utc() -> datetime:
    return datetime.now().astimezone().replace(hour=0, minute=0, second=0, microsecond=0).astimezone(timezone.utc)


def money(value) -> float:
    return float(Decimal(str(value or 0)).quantize(Decimal("0.01"), rounding=ROUND_HALF_UP))


def rows(cursor) -> list[dict]:
    return [dict(r) for r in cursor]


def get_text(data: dict, key: str, label: str, required: bool = True, max_len: int = 200) -> str:
    value = data.get(key)
    if value is None:
        value = ""
    if isinstance(value, bool) or not isinstance(value, (str, int, float)):
        raise ApiError(400, f"{label} is invalid.")
    value = str(value).strip()
    if required and not value:
        raise ApiError(400, f"{label} is required.")
    if len(value) > max_len:
        raise ApiError(400, f"{label} must be at most {max_len} characters.")
    return value


def get_number(data: dict, key: str, label: str, minimum: float = 0, maximum: float = 1e9,
               default=None, integer: bool = False):
    value = data.get(key)
    if value is None or value == "":
        if default is None:
            raise ApiError(400, f"{label} is required.")
        value = default
    try:
        num = float(value)
    except (TypeError, ValueError):
        raise ApiError(400, f"{label} must be a number.") from None
    if not math.isfinite(num) or num < minimum or num > maximum:
        raise ApiError(400, f"{label} must be between {minimum:g} and {maximum:g}.")
    if integer:
        if num != int(num):
            raise ApiError(400, f"{label} must be a whole number.")
        return int(num)
    return num


def get_optional_id(data: dict, key: str, label: str) -> int | None:
    if data.get(key) in (None, ""):
        return None
    return get_number(data, key, label, 1, 2**62, integer=True)


def get_bool(data: dict, key: str, default: bool = False) -> bool:
    value = data.get(key, default)
    if isinstance(value, str):
        return value.strip().lower() in ("1", "true", "yes", "on")
    return bool(value)


def qparam(query: dict, key: str, default: str = "") -> str:
    return (query.get(key) or [default])[0].strip()


def qdate(query: dict, key: str) -> datetime | None:
    value = qparam(query, key)
    if not value:
        return None
    try:
        return parse_iso(value)
    except ValueError:
        raise ApiError(400, f"Invalid date for '{key}'.") from None


def qlimit(query: dict, default: int, maximum: int) -> int:
    try:
        return max(1, min(int(qparam(query, "limit", str(default))), maximum))
    except ValueError:
        return default


def search_clause(columns: list[str], term: str) -> tuple[str, list[str]]:
    pattern = "%" + term.replace("\\", "\\\\").replace("%", "\\%").replace("_", "\\_") + "%"
    return "(" + " OR ".join(f"{c} LIKE ? ESCAPE '\\'" for c in columns) + ")", [pattern] * len(columns)


def csv_bytes(header: list[str], data: list[list]) -> bytes:
    def safe(value):
        # Neutralise spreadsheet formula injection.
        if isinstance(value, str) and value[:1] in ("=", "+", "-", "@", "\t", "\r"):
            return "'" + value
        return value

    buf = io.StringIO()
    writer = csv.writer(buf)
    writer.writerow(header)
    for row in data:
        writer.writerow([safe(v) for v in row])
    return ("\ufeff" + buf.getvalue()).encode("utf-8")


# ---------------------------------------------------------------- authentication

AUTH_LOCK = threading.Lock()
LOGIN_SESSIONS: dict[str, tuple[str, float]] = {}
FAILED_LOGINS = {"count": 0, "locked_until": 0.0}


def hash_password(password: str) -> str:
    salt = secrets.token_bytes(16)
    digest = hashlib.pbkdf2_hmac("sha256", password.encode("utf-8"), salt, PBKDF2_ITERATIONS)
    return f"pbkdf2_sha256${PBKDF2_ITERATIONS}${salt.hex()}${digest.hex()}"


def verify_password(password: str, stored: str) -> bool:
    try:
        algorithm, iterations, salt, digest = stored.split("$")
        # Bound iterations so a tampered backup can't make logins hang.
        if algorithm != "pbkdf2_sha256" or not 1 <= int(iterations) <= 5_000_000:
            return False
        test = hashlib.pbkdf2_hmac("sha256", password.encode("utf-8"), bytes.fromhex(salt), int(iterations))
    except (ValueError, TypeError, AttributeError):
        return False
    return hmac.compare_digest(test.hex(), digest)


def is_default_password(password: str) -> bool:
    return hmac.compare_digest(password.encode("utf-8"), DEFAULT_PASSWORD.encode("utf-8"))


def get_password(data: dict, key: str, label: str) -> str:
    value = data.get(key)
    if not isinstance(value, str) or not value:
        raise ApiError(400, f"{label} is required.")
    if len(value) > 128:
        raise ApiError(400, f"{label} must be at most 128 characters.")
    return value


def check_new_password(password: str) -> None:
    if len(password) < 8 or not re.search(r"[A-Za-z]", password) or not re.search(r"\d", password):
        raise ApiError(400, "New password must be at least 8 characters and include a letter and a number.")


def check_lockout() -> None:
    with AUTH_LOCK:
        wait = FAILED_LOGINS["locked_until"] - time.monotonic()
    if wait > 0:
        raise ApiError(429, f"Too many failed attempts. Try again in {math.ceil(wait)} seconds.")


def record_login_attempt(success: bool) -> None:
    with AUTH_LOCK:
        if success:
            FAILED_LOGINS["count"] = 0
            return
        FAILED_LOGINS["count"] += 1
        if FAILED_LOGINS["count"] >= MAX_FAILED_LOGINS:
            FAILED_LOGINS["count"] = 0
            FAILED_LOGINS["locked_until"] = time.monotonic() + LOCKOUT_SECONDS


def start_login_session(username: str) -> str:
    token = secrets.token_urlsafe(32)
    now = time.monotonic()
    with AUTH_LOCK:
        for expired in [t for t, (_, exp) in LOGIN_SESSIONS.items() if exp < now]:
            del LOGIN_SESSIONS[expired]
        LOGIN_SESSIONS[token] = (username, now + SESSION_TTL)
    return token


def login_session_user(token: str | None) -> str | None:
    if not token:
        return None
    now = time.monotonic()
    with AUTH_LOCK:
        entry = LOGIN_SESSIONS.get(token)
        if entry is None or entry[1] < now:
            LOGIN_SESSIONS.pop(token, None)
            return None
        LOGIN_SESSIONS[token] = (entry[0], now + SESSION_TTL)
        return entry[0]


def end_login_sessions(keep: str | None = None) -> None:
    with AUTH_LOCK:
        for token in list(LOGIN_SESSIONS):
            if token != keep:
                del LOGIN_SESSIONS[token]


def session_cookie(token: str = "") -> str:
    expiry = "" if token else "; Max-Age=0"
    return f"{SESSION_COOKIE}={token}; Path=/; HttpOnly; SameSite=Strict{expiry}"


# ---------------------------------------------------------------- database

def connect() -> sqlite3.Connection:
    conn = sqlite3.connect(DB_PATH, timeout=15)
    conn.row_factory = sqlite3.Row
    conn.execute("PRAGMA foreign_keys = ON")
    return conn


@contextmanager
def db():
    conn = connect()
    try:
        yield conn
        conn.commit()
    except Exception:
        conn.rollback()
        raise
    finally:
        conn.close()


def init_db() -> None:
    DATA_DIR.mkdir(parents=True, exist_ok=True)
    with db() as conn:
        conn.execute("PRAGMA journal_mode = WAL")
        conn.executescript(SCHEMA)
        for key, value in SETTINGS_DEFAULTS.items():
            conn.execute("INSERT OR IGNORE INTO settings (key, value) VALUES (?, ?)", (key, value))
        if conn.execute("SELECT 1 FROM users LIMIT 1").fetchone() is None:
            conn.execute(
                "INSERT INTO users (username, password_hash, default_password, updated_at) VALUES (?, ?, 1, ?)",
                (DEFAULT_USERNAME, hash_password(DEFAULT_PASSWORD), iso(now_utc())),
            )
        if conn.execute("SELECT 1 FROM settings WHERE key = 'seeded'").fetchone() is None:
            conn.executemany(
                "INSERT OR IGNORE INTO rate_plans (name, description, rate_per_hour, minimum_minutes, "
                "increment_minutes, max_charge, is_default) VALUES (?, ?, ?, ?, ?, ?, ?)",
                [
                    ("Standard", "Regular members", 60, 30, 15, 0, 1),
                    ("Student", "Students with valid ID", 40, 30, 15, 0, 0),
                    ("Senior Citizen", "Members aged 60+", 30, 15, 15, 0, 0),
                ],
            )
            conn.executemany(
                "INSERT OR IGNORE INTO computers (name, location) VALUES (?, ?)",
                [(f"PC-{i:02d}", "Main hall") for i in range(1, 7)],
            )
            conn.execute("INSERT INTO settings (key, value) VALUES ('seeded', '1')")


def load_settings(conn) -> dict:
    values = dict(SETTINGS_DEFAULTS)
    values.update({r["key"]: r["value"] for r in conn.execute("SELECT key, value FROM settings")})
    return values


def integrity_message(exc: sqlite3.IntegrityError) -> str:
    text = str(exc)
    if "members.id_number" in text:
        return "A member with this ID number already exists."
    if "computers.name" in text:
        return "A computer with this name already exists."
    if "rate_plans.name" in text:
        return "A rate plan with this name already exists."
    if "sessions.member_id" in text:
        return "This member is already checked in."
    if "sessions.computer_id" in text:
        return "This computer is already in use."
    if "FOREIGN KEY" in text:
        return "This record is referenced by other records and cannot be changed."
    return "The change conflicts with existing data."


# ---------------------------------------------------------------- billing

def compute_bill(session: dict, end: datetime, discount: float = 0.0) -> dict:
    elapsed = max(0, int((end - parse_iso(session["check_in"])).total_seconds()))
    used = math.ceil(elapsed / 60)
    step = max(1, int(session["increment_minutes"]))
    billable = max(math.ceil(used / step) * step, int(session["minimum_minutes"]))
    subtotal = money(billable * float(session["rate_per_hour"]) / 60)
    if session["max_charge"] and session["max_charge"] > 0:
        subtotal = min(subtotal, money(session["max_charge"]))
    discount = money(min(max(discount, 0.0), subtotal))
    tax = money((subtotal - discount) * float(session["tax_percent"]) / 100)
    return {
        "check_out": iso(end),
        "elapsed_seconds": elapsed,
        "used_minutes": used,
        "billable_minutes": billable,
        "subtotal": subtotal,
        "discount": discount,
        "tax": tax,
        "total": money(subtotal - discount + tax),
    }


# ---------------------------------------------------------------- routes

ROUTES: list = []


def route(method: str, pattern: str, upload: bool = False, public: bool = False):
    def register(fn):
        ROUTES.append((method, re.compile(pattern), fn, upload, public))
        return fn
    return register


@route("GET", r"/api/health", public=True)
def health(req):
    return {"app": APP_ID, "version": VERSION}


# --- authentication

def auth_status_payload(username: str | None) -> dict:
    with db() as conn:
        library_name = load_settings(conn)["library_name"]
        user = None
        if username:
            user = conn.execute(
                "SELECT username, default_password FROM users WHERE username = ?", (username,)
            ).fetchone()
    if user is None:
        return {"authenticated": False, "library_name": library_name}
    return {"authenticated": True, "library_name": library_name, "username": user["username"],
            "default_password": bool(user["default_password"])}


@route("GET", r"/api/auth/status", public=True)
def auth_status(req):
    return auth_status_payload(req.handler.user)


@route("POST", r"/api/auth/login", public=True)
def login(req):
    check_lockout()
    username = get_text(req.body, "username", "Username", max_len=80)
    password = get_password(req.body, "password", "Password")
    with db() as conn:
        user = conn.execute("SELECT username, password_hash FROM users WHERE username = ?", (username,)).fetchone()
    ok = user is not None and verify_password(password, user["password_hash"])
    record_login_attempt(ok)
    if not ok:
        raise ApiError(401, "Incorrect username or password.")
    req.handler.pending_headers.append(("Set-Cookie", session_cookie(start_login_session(user["username"]))))
    return auth_status_payload(user["username"])


@route("POST", r"/api/auth/logout", public=True)
def logout(req):
    token = req.handler.session_token()
    if token:
        with AUTH_LOCK:
            LOGIN_SESSIONS.pop(token, None)
    req.handler.pending_headers.append(("Set-Cookie", session_cookie()))
    return {"ok": True}


@route("POST", r"/api/auth/change-password", public=True)
def change_password(req):
    check_lockout()
    data = req.body
    username = req.handler.user or get_text(data, "username", "Username", max_len=80)
    current = get_password(data, "current_password", "Current password")
    new = get_password(data, "new_password", "New password")
    check_new_password(new)
    with db() as conn:
        user = conn.execute("SELECT id, password_hash FROM users WHERE username = ?", (username,)).fetchone()
        # By design, the default password also works here so a forgotten password can be reset.
        ok = user is not None and (is_default_password(current) or verify_password(current, user["password_hash"]))
        record_login_attempt(ok)
        if not ok:
            raise ApiError(401, "The current password (or default password) is incorrect.")
        conn.execute(
            "UPDATE users SET password_hash = ?, default_password = ?, updated_at = ? WHERE id = ?",
            (hash_password(new), int(is_default_password(new)), iso(now_utc()), user["id"]),
        )
    end_login_sessions(keep=req.handler.session_token() if req.handler.user else None)
    return {"ok": True}


@route("GET", r"/api/dashboard")
def dashboard(req):
    since = iso(local_midnight_utc())
    with db() as conn:
        def one(sql, *args):
            return conn.execute(sql, args).fetchone()[0]

        return {
            "active_sessions": one("SELECT COUNT(*) FROM sessions WHERE status = 'active'"),
            "computers_total": one("SELECT COUNT(*) FROM computers WHERE active = 1"),
            "computers_free": one(
                "SELECT COUNT(*) FROM computers c WHERE c.active = 1 AND NOT EXISTS "
                "(SELECT 1 FROM sessions s WHERE s.computer_id = c.id AND s.status = 'active')"
            ),
            "members": one("SELECT COUNT(*) FROM members"),
            "today_checkins": one(
                "SELECT COUNT(*) FROM sessions WHERE check_in >= ? AND status != 'cancelled'", since
            ),
            "today_completed": one(
                "SELECT COUNT(*) FROM sessions WHERE status = 'completed' AND check_out >= ?", since
            ),
            "today_minutes": one(
                "SELECT COALESCE(SUM(duration_minutes), 0) FROM sessions WHERE status = 'completed' AND check_out >= ?",
                since,
            ),
            "today_revenue": money(one(
                "SELECT COALESCE(SUM(total), 0) FROM sessions WHERE status = 'completed' AND check_out >= ?", since
            )),
        }


# --- settings

@route("GET", r"/api/settings")
def get_settings(req):
    with db() as conn:
        s = load_settings(conn)
    return {
        "library_name": s["library_name"],
        "currency": s["currency"],
        "tax_percent": float(s["tax_percent"]),
        "payment_methods": PAYMENT_METHODS,
        "data_dir": str(DATA_DIR),
        "version": VERSION,
    }


@route("PUT", r"/api/settings")
def update_settings(req):
    data = req.body
    updates = {}
    if "library_name" in data:
        updates["library_name"] = get_text(data, "library_name", "Library name", max_len=80)
    if "currency" in data:
        updates["currency"] = get_text(data, "currency", "Currency symbol", max_len=5)
    if "tax_percent" in data:
        updates["tax_percent"] = str(get_number(data, "tax_percent", "Tax percent", 0, 100))
    with db() as conn:
        conn.executemany(
            "INSERT INTO settings (key, value) VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            list(updates.items()),
        )
    return get_settings(req)


# --- members

MEMBER_SELECT = """
SELECT m.*,
    (SELECT COUNT(*) FROM sessions s WHERE s.member_id = m.id AND s.status = 'completed') AS visits,
    (SELECT COALESCE(SUM(s.total), 0) FROM sessions s WHERE s.member_id = m.id AND s.status = 'completed') AS total_spent,
    (SELECT MAX(s.check_in) FROM sessions s WHERE s.member_id = m.id) AS last_visit,
    (SELECT s.id FROM sessions s WHERE s.member_id = m.id AND s.status = 'active') AS active_session_id
FROM members m
"""


def member_input(data: dict) -> dict:
    m = {
        "first_name": get_text(data, "first_name", "First name", max_len=80),
        "last_name": get_text(data, "last_name", "Last name", max_len=80),
        "email": get_text(data, "email", "Email", max_len=160).lower(),
        "phone": get_text(data, "phone", "Phone", max_len=25),
        "id_number": get_text(data, "id_number", "ID number", max_len=60),
        "notes": get_text(data, "notes", "Notes", required=False, max_len=1000),
    }
    if not EMAIL_RE.match(m["email"]):
        raise ApiError(400, "Enter a valid email address.")
    if not PHONE_RE.match(m["phone"]) or sum(c.isdigit() for c in m["phone"]) < 6:
        raise ApiError(400, "Enter a valid phone number.")
    return m


def fetch_member(conn, member_id: int) -> dict:
    row = conn.execute(MEMBER_SELECT + " WHERE m.id = ?", (member_id,)).fetchone()
    if row is None:
        raise ApiError(404, "Member not found.")
    return dict(row)


@route("GET", r"/api/members")
def list_members(req):
    sql, args = MEMBER_SELECT, []
    term = qparam(req.query, "q")
    if term:
        clause, args = search_clause(
            ["m.first_name", "m.last_name", "m.first_name || ' ' || m.last_name",
             "m.email", "m.phone", "m.id_number"], term)
        sql += " WHERE " + clause
    sql += " ORDER BY m.first_name COLLATE NOCASE, m.last_name COLLATE NOCASE LIMIT ?"
    args.append(qlimit(req.query, 1000, 5000))
    with db() as conn:
        return {"items": rows(conn.execute(sql, args))}


@route("GET", r"/api/members/(\d+)")
def get_member(req):
    with db() as conn:
        return fetch_member(conn, req.id())


@route("POST", r"/api/members")
def create_member(req):
    m = member_input(req.body)
    stamp = iso(now_utc())
    with db() as conn:
        cur = conn.execute(
            "INSERT INTO members (first_name, last_name, email, phone, id_number, notes, created_at, updated_at) "
            "VALUES (:first_name, :last_name, :email, :phone, :id_number, :notes, :stamp, :stamp)",
            {**m, "stamp": stamp},
        )
        return fetch_member(conn, cur.lastrowid)


@route("PUT", r"/api/members/(\d+)")
def update_member(req):
    m = member_input(req.body)
    with db() as conn:
        cur = conn.execute(
            "UPDATE members SET first_name = :first_name, last_name = :last_name, email = :email, phone = :phone, "
            "id_number = :id_number, notes = :notes, updated_at = :stamp WHERE id = :id",
            {**m, "stamp": iso(now_utc()), "id": req.id()},
        )
        if cur.rowcount == 0:
            raise ApiError(404, "Member not found.")
        return fetch_member(conn, req.id())


@route("DELETE", r"/api/members/(\d+)")
def delete_member(req):
    with db() as conn:
        fetch_member(conn, req.id())
        if conn.execute("SELECT 1 FROM sessions WHERE member_id = ? LIMIT 1", (req.id(),)).fetchone():
            raise ApiError(409, "This member has session history and cannot be deleted.")
        conn.execute("DELETE FROM members WHERE id = ?", (req.id(),))
    return {"ok": True}


# --- computers

COMPUTER_SELECT = """
SELECT c.*, s.id AS session_id, s.check_in AS session_check_in,
       m.first_name || ' ' || m.last_name AS session_member
FROM computers c
LEFT JOIN sessions s ON s.computer_id = c.id AND s.status = 'active'
LEFT JOIN members m ON m.id = s.member_id
"""


def computer_input(data: dict) -> dict:
    return {
        "name": get_text(data, "name", "Computer name", max_len=60),
        "location": get_text(data, "location", "Location", required=False, max_len=120),
        "active": 1 if get_bool(data, "active", True) else 0,
    }


def fetch_computer(conn, computer_id: int) -> dict:
    row = conn.execute(COMPUTER_SELECT + " WHERE c.id = ?", (computer_id,)).fetchone()
    if row is None:
        raise ApiError(404, "Computer not found.")
    return dict(row)


@route("GET", r"/api/computers")
def list_computers(req):
    with db() as conn:
        return {"items": rows(conn.execute(COMPUTER_SELECT + " ORDER BY c.name COLLATE NOCASE"))}


@route("POST", r"/api/computers")
def create_computer(req):
    c = computer_input(req.body)
    with db() as conn:
        cur = conn.execute("INSERT INTO computers (name, location, active) VALUES (:name, :location, :active)", c)
        return fetch_computer(conn, cur.lastrowid)


@route("PUT", r"/api/computers/(\d+)")
def update_computer(req):
    c = computer_input(req.body)
    with db() as conn:
        current = fetch_computer(conn, req.id())
        if current["session_id"] and not c["active"]:
            raise ApiError(409, "This computer is in use. Check the member out before disabling it.")
        conn.execute(
            "UPDATE computers SET name = :name, location = :location, active = :active WHERE id = :id",
            {**c, "id": req.id()},
        )
        return fetch_computer(conn, req.id())


@route("DELETE", r"/api/computers/(\d+)")
def delete_computer(req):
    with db() as conn:
        if fetch_computer(conn, req.id())["session_id"]:
            raise ApiError(409, "This computer is in use. Check the member out before deleting it.")
        conn.execute("DELETE FROM computers WHERE id = ?", (req.id(),))
    return {"ok": True}


# --- rate card

def rate_input(data: dict) -> dict:
    return {
        "name": get_text(data, "name", "Plan name", max_len=60),
        "description": get_text(data, "description", "Description", required=False, max_len=200),
        "rate_per_hour": get_number(data, "rate_per_hour", "Rate per hour", 0, 1e7),
        "minimum_minutes": get_number(data, "minimum_minutes", "Minimum minutes", 0, 1440, default=0, integer=True),
        "increment_minutes": get_number(data, "increment_minutes", "Billing increment", 1, 1440, default=1, integer=True),
        "max_charge": get_number(data, "max_charge", "Maximum charge", 0, 1e9, default=0),
        "active": 1 if get_bool(data, "active", True) else 0,
        "is_default": 1 if get_bool(data, "is_default", False) else 0,
    }


def fetch_rate(conn, rate_id: int) -> dict:
    row = conn.execute("SELECT * FROM rate_plans WHERE id = ?", (rate_id,)).fetchone()
    if row is None:
        raise ApiError(404, "Rate plan not found.")
    return dict(row)


def normalise_default(conn, preferred_id: int | None = None) -> None:
    if preferred_id is not None:
        conn.execute("UPDATE rate_plans SET is_default = (id = ?)", (preferred_id,))
    conn.execute("UPDATE rate_plans SET is_default = 0 WHERE active = 0")
    if conn.execute("SELECT 1 FROM rate_plans WHERE is_default = 1").fetchone() is None:
        conn.execute(
            "UPDATE rate_plans SET is_default = 1 WHERE id = "
            "(SELECT id FROM rate_plans WHERE active = 1 ORDER BY id LIMIT 1)"
        )


@route("GET", r"/api/rates")
def list_rates(req):
    with db() as conn:
        return {"items": rows(conn.execute(
            "SELECT * FROM rate_plans ORDER BY is_default DESC, active DESC, name COLLATE NOCASE"
        ))}


@route("POST", r"/api/rates")
def create_rate(req):
    r = rate_input(req.body)
    with db() as conn:
        cur = conn.execute(
            "INSERT INTO rate_plans (name, description, rate_per_hour, minimum_minutes, increment_minutes, "
            "max_charge, active, is_default) VALUES (:name, :description, :rate_per_hour, :minimum_minutes, "
            ":increment_minutes, :max_charge, :active, 0)",
            r,
        )
        normalise_default(conn, cur.lastrowid if r["is_default"] and r["active"] else None)
        return fetch_rate(conn, cur.lastrowid)


@route("PUT", r"/api/rates/(\d+)")
def update_rate(req):
    r = rate_input(req.body)
    with db() as conn:
        fetch_rate(conn, req.id())
        conn.execute(
            "UPDATE rate_plans SET name = :name, description = :description, rate_per_hour = :rate_per_hour, "
            "minimum_minutes = :minimum_minutes, increment_minutes = :increment_minutes, max_charge = :max_charge, "
            "active = :active WHERE id = :id",
            {**r, "id": req.id()},
        )
        normalise_default(conn, req.id() if r["is_default"] and r["active"] else None)
        return fetch_rate(conn, req.id())


@route("DELETE", r"/api/rates/(\d+)")
def delete_rate(req):
    with db() as conn:
        fetch_rate(conn, req.id())
        conn.execute("DELETE FROM rate_plans WHERE id = ?", (req.id(),))
        normalise_default(conn)
    return {"ok": True}


# --- sessions

SESSION_SELECT = """
SELECT s.*, m.first_name, m.last_name, m.email, m.phone, m.id_number
FROM sessions s JOIN members m ON m.id = s.member_id
"""


def fetch_session(conn, session_id: int) -> dict:
    row = conn.execute(SESSION_SELECT + " WHERE s.id = ?", (session_id,)).fetchone()
    if row is None:
        raise ApiError(404, "Session not found.")
    return dict(row)


def session_filters(query: dict) -> tuple[str, list]:
    where, args = [], []
    status = qparam(query, "status")
    if status in ("active", "completed", "cancelled"):
        where.append("s.status = ?")
        args.append(status)
    start, end = qdate(query, "from"), qdate(query, "to")
    if start:
        where.append("s.check_in >= ?")
        args.append(iso(start))
    if end:
        where.append("s.check_in < ?")
        args.append(iso(end))
    member_id = qparam(query, "member_id")
    if member_id.isdigit():
        where.append("s.member_id = ?")
        args.append(int(member_id))
    term = qparam(query, "q")
    if term:
        clause, extra = search_clause(
            ["m.first_name", "m.last_name", "m.first_name || ' ' || m.last_name", "m.email",
             "m.phone", "m.id_number", "s.computer_name", "s.plan_name"], term)
        where.append(clause)
        args.extend(extra)
    return (" WHERE " + " AND ".join(where)) if where else "", args


@route("GET", r"/api/sessions")
def list_sessions(req):
    where, args = session_filters(req.query)
    args.append(qlimit(req.query, 1000, 10000))
    with db() as conn:
        items = rows(conn.execute(SESSION_SELECT + where + " ORDER BY s.check_in DESC LIMIT ?", args))
    return {"items": items, "server_now": datetime.now(timezone.utc).isoformat(timespec="milliseconds")}


@route("GET", r"/api/sessions/(\d+)")
def get_session(req):
    with db() as conn:
        return fetch_session(conn, req.id())


@route("POST", r"/api/sessions/checkin")
def check_in(req):
    data = req.body
    member_id = get_number(data, "member_id", "Member", 1, 2**62, integer=True)
    computer_id = get_optional_id(data, "computer_id", "Computer")
    rate_plan_id = get_optional_id(data, "rate_plan_id", "Rate plan")
    notes = get_text(data, "notes", "Notes", required=False, max_len=500)
    with db() as conn:
        member = fetch_member(conn, member_id)
        if member["active_session_id"]:
            raise ApiError(409, f"{member['first_name']} {member['last_name']} is already checked in.")

        computer_name = ""
        if computer_id is not None:
            computer = fetch_computer(conn, computer_id)
            if not computer["active"]:
                raise ApiError(409, f"{computer['name']} is disabled.")
            if computer["session_id"]:
                raise ApiError(409, f"{computer['name']} is already in use by {computer['session_member']}.")
            computer_name = computer["name"]

        if rate_plan_id is None:
            plan = conn.execute("SELECT * FROM rate_plans WHERE is_default = 1 AND active = 1").fetchone()
            if plan is None:
                raise ApiError(409, "No default rate plan is set. Update the rate card first.")
            plan = dict(plan)
        else:
            plan = fetch_rate(conn, rate_plan_id)
            if not plan["active"]:
                raise ApiError(409, f"Rate plan '{plan['name']}' is inactive.")

        tax_percent = float(load_settings(conn)["tax_percent"])
        cur = conn.execute(
            "INSERT INTO sessions (member_id, computer_id, computer_name, rate_plan_id, plan_name, rate_per_hour, "
            "minimum_minutes, increment_minutes, max_charge, tax_percent, check_in, notes) "
            "VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            (member_id, computer_id, computer_name, plan["id"], plan["name"], plan["rate_per_hour"],
             plan["minimum_minutes"], plan["increment_minutes"], plan["max_charge"], tax_percent,
             iso(now_utc()), notes),
        )
        return fetch_session(conn, cur.lastrowid)


def active_session(conn, session_id: int) -> dict:
    session = fetch_session(conn, session_id)
    if session["status"] != "active":
        raise ApiError(409, "This session is already closed.")
    return session


@route("GET", r"/api/sessions/(\d+)/quote")
def quote(req):
    discount = get_number({"d": qparam(req.query, "discount")}, "d", "Discount", 0, 1e9, default=0)
    with db() as conn:
        session = active_session(conn, req.id())
    return {"session": session, "bill": compute_bill(session, now_utc(), discount)}


@route("POST", r"/api/sessions/(\d+)/checkout")
def check_out(req):
    data = req.body
    discount = get_number(data, "discount", "Discount", 0, 1e9, default=0)
    payment = get_text(data, "payment_method", "Payment method", max_len=30)
    if payment not in PAYMENT_METHODS:
        raise ApiError(400, "Choose a valid payment method.")
    with db() as conn:
        session = active_session(conn, req.id())
        notes = get_text(data, "notes", "Notes", required=False, max_len=500) if "notes" in data else session["notes"]
        bill = compute_bill(session, now_utc(), discount)
        cur = conn.execute(
            "UPDATE sessions SET check_out = ?, duration_minutes = ?, billable_minutes = ?, subtotal = ?, "
            "discount = ?, tax = ?, total = ?, payment_method = ?, notes = ?, status = 'completed' "
            "WHERE id = ? AND status = 'active'",
            (bill["check_out"], bill["used_minutes"], bill["billable_minutes"], bill["subtotal"],
             bill["discount"], bill["tax"], bill["total"], payment, notes, req.id()),
        )
        if cur.rowcount == 0:
            raise ApiError(409, "This session is already closed.")
        return fetch_session(conn, req.id())


@route("POST", r"/api/sessions/(\d+)/cancel")
def cancel_session(req):
    with db() as conn:
        session = active_session(conn, req.id())
        end = now_utc()
        used = math.ceil(max(0, (end - parse_iso(session["check_in"])).total_seconds()) / 60)
        conn.execute(
            "UPDATE sessions SET status = 'cancelled', check_out = ?, duration_minutes = ?, billable_minutes = 0, "
            "subtotal = 0, discount = 0, tax = 0, total = 0 WHERE id = ? AND status = 'active'",
            (iso(end), used, req.id()),
        )
        return fetch_session(conn, req.id())


# --- reports

@route("GET", r"/api/reports")
def reports(req):
    start = qdate(req.query, "from") or local_midnight_utc()
    end = qdate(req.query, "to") or (now_utc() + timedelta(days=1))
    with db() as conn:
        items = rows(conn.execute(
            SESSION_SELECT + " WHERE s.status = 'completed' AND s.check_out >= ? AND s.check_out < ? "
            "ORDER BY s.check_out",
            (iso(start), iso(end)),
        ))

    def bucket(store: dict, key: str, **extra):
        entry = store.setdefault(key, {"name": key, "sessions": 0, "minutes": 0, "revenue": 0.0, **extra})
        return entry

    by_day, by_payment, by_plan, by_member = {}, {}, {}, {}
    totals = {"sessions": 0, "minutes": 0, "billable_minutes": 0, "subtotal": 0.0, "discount": 0.0, "tax": 0.0,
              "revenue": 0.0}
    for s in items:
        totals["sessions"] += 1
        totals["minutes"] += s["duration_minutes"] or 0
        totals["billable_minutes"] += s["billable_minutes"] or 0
        for k, col in (("subtotal", "subtotal"), ("discount", "discount"), ("tax", "tax"), ("revenue", "total")):
            totals[k] += s[col] or 0
        day = parse_iso(s["check_out"]).astimezone().strftime("%Y-%m-%d")
        member_key = f"{s['first_name']} {s['last_name']} ({s['id_number']})"
        for entry in (
            bucket(by_day, day),
            bucket(by_payment, s["payment_method"] or "Unknown"),
            bucket(by_plan, s["plan_name"]),
            bucket(by_member, member_key, member_id=s["member_id"]),
        ):
            entry["sessions"] += 1
            entry["minutes"] += s["duration_minutes"] or 0
            entry["revenue"] += s["total"] or 0

    def finish(store: dict, key=None, reverse=True, limit=None):
        out = sorted(store.values(), key=key or (lambda e: e["revenue"]), reverse=reverse)
        for e in out:
            e["revenue"] = money(e["revenue"])
        return out[:limit] if limit else out

    for k in ("subtotal", "discount", "tax", "revenue"):
        totals[k] = money(totals[k])
    count = totals["sessions"] or 1
    totals["avg_minutes"] = round(totals["minutes"] / count, 1) if totals["sessions"] else 0
    totals["avg_revenue"] = money(totals["revenue"] / count) if totals["sessions"] else 0
    return {
        "from": iso(start),
        "to": iso(end),
        "summary": totals,
        "by_day": finish(by_day, key=lambda e: e["name"], reverse=False),
        "by_payment": finish(by_payment),
        "by_plan": finish(by_plan),
        "top_members": finish(by_member, limit=10),
    }


# --- exports and maintenance

@route("GET", r"/api/export/members\.csv")
def export_members(req):
    with db() as conn:
        items = rows(conn.execute(MEMBER_SELECT + " ORDER BY m.first_name COLLATE NOCASE, m.last_name COLLATE NOCASE"))
    data = [[m["id"], m["first_name"], m["last_name"], m["email"], m["phone"], m["id_number"], m["visits"],
             money(m["total_spent"]), local_text(m["last_visit"]), local_text(m["created_at"]), m["notes"]]
            for m in items]
    header = ["Member #", "First name", "Last name", "Email", "Phone", "ID number", "Visits", "Total spent",
              "Last visit", "Registered", "Notes"]
    return Raw(csv_bytes(header, data), "text/csv; charset=utf-8", f"members-{datetime.now():%Y%m%d}.csv")


@route("GET", r"/api/export/sessions\.csv")
def export_sessions(req):
    where, args = session_filters(req.query)
    with db() as conn:
        items = rows(conn.execute(SESSION_SELECT + where + " ORDER BY s.check_in DESC", args))
    data = [[s["id"], s["first_name"], s["last_name"], s["id_number"], s["phone"], s["email"], s["computer_name"],
             s["plan_name"], s["rate_per_hour"], local_text(s["check_in"]), local_text(s["check_out"]),
             s["duration_minutes"], s["billable_minutes"], s["subtotal"], s["discount"], s["tax"], s["total"],
             s["payment_method"], s["status"], s["notes"]] for s in items]
    header = ["Receipt #", "First name", "Last name", "ID number", "Phone", "Email", "Computer", "Plan",
              "Rate per hour", "Check in", "Check out", "Minutes used", "Minutes billed", "Subtotal", "Discount",
              "Tax", "Total", "Payment", "Status", "Notes"]
    return Raw(csv_bytes(header, data), "text/csv; charset=utf-8", f"sessions-{datetime.now():%Y%m%d}.csv")


@route("GET", r"/api/backup")
def backup(req):
    fd, tmp = tempfile.mkstemp(suffix=".db")
    os.close(fd)
    try:
        src, dst = connect(), sqlite3.connect(tmp)
        try:
            src.backup(dst)
        finally:
            dst.close()
            src.close()
        body = Path(tmp).read_bytes()
    finally:
        os.unlink(tmp)
    return Raw(body, "application/vnd.sqlite3", f"library-backup-{datetime.now():%Y%m%d-%H%M}.db")


@route("POST", r"/api/restore", upload=True)
def restore(req):
    invalid = "This is not a Digital Library backup file."
    if not req.body.startswith(SQLITE_MAGIC):
        raise ApiError(400, invalid)
    fd, tmp = tempfile.mkstemp(suffix=".db")
    try:
        with os.fdopen(fd, "wb") as fh:
            fh.write(req.body)
        with closing(sqlite3.connect(tmp)) as src:
            try:
                tables = {r[0] for r in src.execute("SELECT name FROM sqlite_master WHERE type = 'table'")}
                if not REQUIRED_TABLES <= tables:
                    raise ApiError(400, invalid)
                if src.execute("PRAGMA integrity_check").fetchone()[0] != "ok":
                    raise ApiError(400, "The backup file is damaged.")
            except sqlite3.DatabaseError:
                raise ApiError(400, invalid) from None
            safety = DATA_DIR / f"library-before-restore-{datetime.now():%Y%m%d-%H%M%S}.db"
            with closing(connect()) as current, closing(sqlite3.connect(safety)) as copy:
                current.backup(copy)
            with closing(connect()) as current:
                src.backup(current)
    finally:
        os.unlink(tmp)
    init_db()
    with db() as conn:
        counts = {t: conn.execute(f"SELECT COUNT(*) FROM {t}").fetchone()[0] for t in ("members", "sessions")}
    # Credentials come from the backup, so everyone signs in again with them.
    end_login_sessions()
    req.handler.pending_headers.append(("Set-Cookie", session_cookie()))
    return {"ok": True, **counts, "previous_data": str(safety), "signed_out": True}


@route("POST", r"/api/shutdown")
def shutdown(req):
    threading.Thread(target=req.handler.server.shutdown, daemon=True).start()
    return {"ok": True}


# ---------------------------------------------------------------- HTTP layer

class Handler(BaseHTTPRequestHandler):
    server_version = f"DigitalLibrary/{VERSION}"

    def log_message(self, fmt, *args):
        if os.environ.get("LIBRARY_DEBUG"):
            super().log_message(fmt, *args)

    def do_GET(self):
        self.dispatch("GET")

    def do_POST(self):
        self.dispatch("POST")

    def do_PUT(self):
        self.dispatch("PUT")

    def do_DELETE(self):
        self.dispatch("DELETE")

    def allowed_hosts(self) -> set[str]:
        port = self.server.server_address[1]
        return {f"127.0.0.1:{port}", f"localhost:{port}"}

    def dispatch(self, method: str) -> None:
        self.pending_headers: list[tuple[str, str]] = []
        self.user: str | None = None
        url = urlparse(self.path)
        # Host check blocks DNS-rebinding; Origin check blocks cross-site writes.
        if self.headers.get("Host", "") not in self.allowed_hosts():
            return self.send_json({"error": "Forbidden host."}, 403)
        if not url.path.startswith("/api/"):
            if method != "GET":
                return self.send_json({"error": "Method not allowed."}, 405)
            return self.serve_static(url.path)
        if method != "GET":
            origin = self.headers.get("Origin")
            if origin and origin not in {f"http://{h}" for h in self.allowed_hosts()}:
                return self.send_json({"error": "Cross-origin request blocked."}, 403)

        for route_method, pattern, fn, upload, public in ROUTES:
            match = pattern.fullmatch(url.path)
            if match and route_method == method:
                break
        else:
            return self.send_json({"error": "Not found."}, 404)

        self.user = login_session_user(self.session_token())
        if not public and self.user is None:
            return self.send_json({"error": "Please sign in."}, 401)

        try:
            if upload:
                body = self.read_upload()
            else:
                body = self.read_body() if method in ("POST", "PUT") else {}
            result = fn(Request(self, match, parse_qs(url.query), body))
        except ApiError as exc:
            return self.send_json({"error": exc.message}, exc.status)
        except sqlite3.IntegrityError as exc:
            return self.send_json({"error": integrity_message(exc)}, 409)
        except Exception:
            traceback.print_exc()
            return self.send_json({"error": "Unexpected server error."}, 500)

        if isinstance(result, Raw):
            self.send_bytes(result.body, result.content_type, filename=result.filename)
        else:
            self.send_json(result)

    def session_token(self) -> str | None:
        cookie = SimpleCookie()
        try:
            cookie.load(self.headers.get("Cookie", ""))
        except CookieError:
            return None
        morsel = cookie.get(SESSION_COOKIE)
        return morsel.value if morsel else None

    def content_length(self, limit: int) -> int:
        try:
            length = int(self.headers.get("Content-Length") or 0)
        except ValueError:
            raise ApiError(400, "Invalid Content-Length.") from None
        if length > limit:
            raise ApiError(413, "Request is too large.")
        return length

    def read_upload(self) -> bytes:
        ctype = self.headers.get("Content-Type", "").split(";")[0].strip().lower()
        if ctype != "application/octet-stream":
            raise ApiError(415, "Upload the file as binary data.")
        length = self.content_length(MAX_UPLOAD)
        if not length:
            raise ApiError(400, "The uploaded file is empty.")
        return self.rfile.read(length)

    def read_body(self) -> dict:
        ctype = self.headers.get("Content-Type", "").split(";")[0].strip().lower()
        if ctype != "application/json":
            raise ApiError(415, "Requests must be sent as JSON.")
        length = self.content_length(MAX_BODY)
        raw = self.rfile.read(length) if length else b"{}"
        try:
            data = json.loads(raw or b"{}")
        except (json.JSONDecodeError, UnicodeDecodeError):
            raise ApiError(400, "Invalid JSON.") from None
        if not isinstance(data, dict):
            raise ApiError(400, "Expected a JSON object.")
        return data

    def serve_static(self, path: str) -> None:
        rel = "index.html" if path in ("", "/") else unquote(path).lstrip("/")
        target = (STATIC_DIR / rel).resolve()
        if STATIC_DIR not in target.parents or not target.is_file():
            return self.send_bytes(b"Not found", "text/plain; charset=utf-8", status=404)
        ctype = mimetypes.guess_type(target.name)[0] or "application/octet-stream"
        if ctype.startswith("text/") or ctype.endswith("javascript"):
            ctype += "; charset=utf-8"
        self.send_bytes(target.read_bytes(), ctype)

    def send_json(self, data, status: int = 200) -> None:
        self.send_bytes(json.dumps(data, ensure_ascii=False).encode("utf-8"),
                        "application/json; charset=utf-8", status=status)

    def send_bytes(self, body: bytes, content_type: str, status: int = 200, filename: str | None = None) -> None:
        self.send_response(status)
        self.send_header("Content-Type", content_type)
        self.send_header("Content-Length", str(len(body)))
        if filename:
            self.send_header("Content-Disposition", f'attachment; filename="{filename}"')
        self.send_header("Cache-Control", "no-store")
        self.send_header("X-Content-Type-Options", "nosniff")
        self.send_header("X-Frame-Options", "DENY")
        self.send_header("Referrer-Policy", "no-referrer")
        self.send_header(
            "Content-Security-Policy",
            "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; "
            "connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'self'",
        )
        for name, value in self.pending_headers:
            self.send_header(name, value)
        self.end_headers()
        self.wfile.write(body)


def is_our_server(port: int) -> bool:
    try:
        with urlopen(f"http://{HOST}:{port}/api/health", timeout=0.5) as resp:
            return json.loads(resp.read()).get("app") == APP_ID
    except Exception:
        return False


def main() -> None:
    open_browser = "--no-browser" not in sys.argv
    if getattr(sys, "frozen", False):
        LOG_PATH.parent.mkdir(parents=True, exist_ok=True)
        sys.stdout = sys.stderr = open(LOG_PATH, "a", buffering=1, encoding="utf-8")
    init_db()

    for port in range(DEFAULT_PORT, DEFAULT_PORT + PORT_RANGE):
        if is_our_server(port):
            url = f"http://{HOST}:{port}/"
            print(f"Digital Library is already running at {url}")
            if open_browser:
                webbrowser.open(url)
            return

    server = None
    for port in range(DEFAULT_PORT, DEFAULT_PORT + PORT_RANGE):
        try:
            server = ThreadingHTTPServer((HOST, port), Handler)
            break
        except OSError:
            continue
    if server is None:
        sys.exit(f"No free port between {DEFAULT_PORT} and {DEFAULT_PORT + PORT_RANGE - 1}.")

    server.daemon_threads = True
    url = f"http://{HOST}:{server.server_address[1]}/"
    print(f"Digital Library running at {url}")
    print(f"Data stored in {DB_PATH}")
    print("Press Ctrl+C to stop.", flush=True)
    if open_browser:
        threading.Timer(0.6, webbrowser.open, [url]).start()
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()
        print("Digital Library stopped.")


if __name__ == "__main__":
    main()
