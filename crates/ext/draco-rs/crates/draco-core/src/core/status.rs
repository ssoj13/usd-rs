//! Status utilities.
//! Reference: `_ref/draco/src/draco/core/status.h` + `.cc`.

use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StatusCode {
    Ok = 0,
    DracoError = -1,
    IoError = -2,
    InvalidParameter = -3,
    UnsupportedVersion = -4,
    UnknownVersion = -5,
    UnsupportedFeature = -6,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Status {
    code: StatusCode,
    error_msg: String,
}

impl Status {
    pub fn ok() -> Self {
        Self::new(StatusCode::Ok, "")
    }

    pub fn error(msg: &str) -> Self {
        Self::new(StatusCode::DracoError, msg)
    }

    pub fn new(code: StatusCode, error_msg: &str) -> Self {
        Self {
            code,
            error_msg: error_msg.to_string(),
        }
    }

    pub fn code(&self) -> StatusCode {
        self.code.clone()
    }

    pub fn error_msg_string(&self) -> &str {
        &self.error_msg
    }

    pub fn error_msg(&self) -> &str {
        &self.error_msg
    }

    pub fn code_string(&self) -> &'static str {
        match self.code {
            StatusCode::Ok => "OK",
            StatusCode::DracoError => "DRACO_ERROR",
            StatusCode::IoError => "IO_ERROR",
            StatusCode::InvalidParameter => "INVALID_PARAMETER",
            StatusCode::UnsupportedVersion => "UNSUPPORTED_VERSION",
            StatusCode::UnknownVersion => "UNKNOWN_VERSION",
            StatusCode::UnsupportedFeature => "UNSUPPORTED_FEATURE",
        }
    }

    pub fn code_and_error_string(&self) -> String {
        format!("{}: {}", self.code_string(), self.error_msg_string())
    }

    pub fn is_ok(&self) -> bool {
        self.code == StatusCode::Ok
    }
}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.error_msg_string())
    }
}

pub fn ok_status() -> Status {
    Status::ok()
}

pub fn error_status(msg: &str) -> Status {
    Status::error(msg)
}

#[macro_export]
macro_rules! draco_return_if_error {
    ($expression:expr) => {{
        let _local_status = $expression;
        if !_local_status.is_ok() {
            return _local_status;
        }
    }};
}
