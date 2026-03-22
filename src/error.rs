use std::error::Error;
use std::fmt::Display;
use std::io;
use std::num::ParseIntError;

use base64::DecodeError;
use grammers_client::InvocationError;
use grammers_client::tl::deserialize;
use ntex::http::StatusCode;
use ntex::web;
use ntex::ws::error::ProtocolError;

#[derive(Debug)]
pub struct WebError {
    code: StatusCode,
    reason: String,
}

impl WebError {
    pub fn new(code: StatusCode, reason: String) -> Self {
        Self { code, reason }
    }
}

impl Display for WebError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.code, self.reason)
    }
}

impl Error for WebError {}

impl web::error::WebResponseError for WebError {
    fn error_response(&self, _: &web::HttpRequest) -> web::HttpResponse {
        web::HttpResponse::build(self.status_code())
            .set_header("content-type", "text/html; charset=utf-8")
            .body(self.reason.clone())
    }

    fn status_code(&self) -> StatusCode {
        self.code
    }
}

impl From<InvocationError> for WebError {
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

impl From<serde_json::Error> for WebError {
    fn from(err: serde_json::Error) -> Self {
        Self {
            code: StatusCode::BAD_REQUEST,
            reason: err.to_string(),
        }
    }
}

impl From<ParseIntError> for WebError {
    fn from(err: ParseIntError) -> Self {
        Self {
            code: StatusCode::BAD_REQUEST,
            reason: err.to_string(),
        }
    }
}

impl From<DecodeError> for WebError {
    fn from(err: DecodeError) -> Self {
        Self {
            code: StatusCode::BAD_REQUEST,
            reason: err.to_string(),
        }
    }
}

impl From<deserialize::Error> for WebError {
    fn from(err: deserialize::Error) -> Self {
        Self {
            code: StatusCode::BAD_REQUEST,
            reason: err.to_string(),
        }
    }
}

impl From<ProtocolError> for WebError {
    fn from(err: ProtocolError) -> Self {
        Self {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            reason: err.to_string(),
        }
    }
}

impl From<io::Error> for WebError {
    fn from(err: io::Error) -> Self {
        Self {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            reason: err.to_string(),
        }
    }
}
