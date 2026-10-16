use std::error::Error;
use std::fmt::Display;
use std::io;
use std::num::ParseIntError;

use base64::DecodeError;
use grammers_client::InvocationError;
use grammers_client::tl::deserialize;
use ntex::http::StatusCode;
use ntex::web::error::{JsonPayloadError, PathError};
use ntex::web::{HttpResponse, HttpResponseBuilder, WebResponseError};
use ntex::ws::error::ProtocolError;
use ntex_multipart::MultipartError;

use crate::state::{AppErrors, AppState};

#[derive(Debug)]
pub struct AppError {
    code: StatusCode,
    reason: String,
}

impl AppError {
    pub fn new(code: StatusCode, reason: String) -> Self {
        Self { code, reason }
    }
}

impl Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.code, self.reason)
    }
}

impl Error for AppError {}

impl WebResponseError<AppState, AppErrors> for AppError {
    fn error_response(&self, _st: &AppState) -> HttpResponse {
        HttpResponseBuilder::new(self.code)
            .header("content-type", "text/html; charset=utf-8")
            .body(self.reason.clone())
    }
}

impl WebResponseError<AppState, AppErrors> for JsonPayloadError {
    fn error_response(&self, _st: &AppState) -> HttpResponse {
        let code = match self {
            JsonPayloadError::Overflow => StatusCode::PAYLOAD_TOO_LARGE,
            _ => StatusCode::BAD_REQUEST,
        };
        HttpResponseBuilder::new(code)
            .header("content-type", "text/html; charset=utf-8")
            .body(self.to_string())
    }
}

impl WebResponseError<AppState, AppErrors> for PathError {
    fn error_response(&self, _st: &AppState) -> HttpResponse {
        HttpResponseBuilder::new(StatusCode::BAD_REQUEST)
            .header("content-type", "text/html; charset=utf-8")
            .body(self.to_string())
    }
}

impl From<InvocationError> for AppError {
    fn from(err: InvocationError) -> Self {
        match err {
            InvocationError::Rpc(ref rpc) => Self {
                code: StatusCode::from_u16(rpc.code as u16)
                    .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
                reason: err.to_string(),
            },
            _ => Self {
                code: StatusCode::INTERNAL_SERVER_ERROR,
                reason: err.to_string(),
            },
        }
    }
}

impl From<serde_json::Error> for AppError {
    fn from(err: serde_json::Error) -> Self {
        Self {
            code: StatusCode::BAD_REQUEST,
            reason: err.to_string(),
        }
    }
}

impl From<ParseIntError> for AppError {
    fn from(err: ParseIntError) -> Self {
        Self {
            code: StatusCode::BAD_REQUEST,
            reason: err.to_string(),
        }
    }
}

impl From<DecodeError> for AppError {
    fn from(err: DecodeError) -> Self {
        Self {
            code: StatusCode::BAD_REQUEST,
            reason: err.to_string(),
        }
    }
}

impl From<deserialize::Error> for AppError {
    fn from(err: deserialize::Error) -> Self {
        Self {
            code: StatusCode::BAD_REQUEST,
            reason: err.to_string(),
        }
    }
}

impl From<ProtocolError> for AppError {
    fn from(err: ProtocolError) -> Self {
        Self {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            reason: err.to_string(),
        }
    }
}

impl From<io::Error> for AppError {
    fn from(err: io::Error) -> Self {
        Self {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            reason: err.to_string(),
        }
    }
}

impl From<MultipartError> for AppError {
    fn from(err: MultipartError) -> Self {
        Self {
            code: StatusCode::BAD_REQUEST,
            reason: err.to_string(),
        }
    }
}

impl From<async_tempfile::Error> for AppError {
    fn from(err: async_tempfile::Error) -> Self {
        Self {
            code: match err {
                async_tempfile::Error::InvalidAffix => StatusCode::BAD_REQUEST,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            },
            reason: err.to_string(),
        }
    }
}
