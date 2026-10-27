use serde::Serialize;

/// Error returned to the UI as `{status, error}`; status codes mirror HTTP semantics.
#[derive(Debug, Serialize)]
pub struct ApiError {
    pub status: u16,
    pub error: String,
}

pub type ApiResult<T> = Result<T, ApiError>;

impl ApiError {
    pub fn new(status: u16, message: impl Into<String>) -> Self {
        Self { status, error: message.into() }
    }
}

fn integrity_message(text: &str) -> &'static str {
    if text.contains("members.id_number") {
        "A member with this ID number already exists."
    } else if text.contains("computers.name") {
        "A computer with this name already exists."
    } else if text.contains("rate_plans.name") {
        "A rate plan with this name already exists."
    } else if text.contains("sessions.member_id") {
        "This member is already checked in."
    } else if text.contains("sessions.computer_id") {
        "This computer is already in use."
    } else if text.contains("FOREIGN KEY") {
        "This record is referenced by other records and cannot be changed."
    } else {
        "The change conflicts with existing data."
    }
}

impl From<rusqlite::Error> for ApiError {
    fn from(err: rusqlite::Error) -> Self {
        if let rusqlite::Error::SqliteFailure(code, message) = &err {
            if code.code == rusqlite::ErrorCode::ConstraintViolation {
                return ApiError::new(409, integrity_message(message.as_deref().unwrap_or("")));
            }
        }
        eprintln!("database error: {err}");
        ApiError::new(500, "Unexpected database error.")
    }
}

impl From<std::io::Error> for ApiError {
    fn from(err: std::io::Error) -> Self {
        ApiError::new(500, format!("Could not read or write the file: {err}"))
    }
}
