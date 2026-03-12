use std::{io, num::ParseIntError};

use base64::DecodeError;
use derive_more::Display;
use grammers_client::{InvocationError, tl::deserialize};
use ntex::{http::StatusCode, web, ws::error::ProtocolError};

#[derive(Display, Debug)]
#[display("{code} {reason}")]
pub struct MyError {
    code: StatusCode,
    reason: String,
}

impl MyError {
    pub fn new(code: StatusCode, reason: String) -> Self {
        Self { code, reason }
    }
}

impl web::error::WebResponseError for MyError {
    fn error_response(&self, _: &web::HttpRequest) -> web::HttpResponse {
        web::HttpResponse::build(self.status_code())
            .set_header("content-type", "text/html; charset=utf-8")
            .body(self.reason.clone())
    }

    fn status_code(&self) -> StatusCode {
        self.code
    }
}

impl From<InvocationError> for MyError {
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

impl From<serde_json::Error> for MyError {
    fn from(err: serde_json::Error) -> Self {
        Self {
            code: StatusCode::BAD_REQUEST,
            reason: err.to_string(),
        }
    }
}

impl From<ParseIntError> for MyError {
    fn from(err: ParseIntError) -> Self {
        Self {
            code: StatusCode::BAD_REQUEST,
            reason: err.to_string(),
        }
    }
}

impl From<DecodeError> for MyError {
    fn from(err: DecodeError) -> Self {
        Self {
            code: StatusCode::BAD_REQUEST,
            reason: err.to_string(),
        }
    }
}

impl From<deserialize::Error> for MyError {
    fn from(err: deserialize::Error) -> Self {
        Self {
            code: StatusCode::BAD_REQUEST,
            reason: err.to_string(),
        }
    }
}

impl From<ProtocolError> for MyError {
    fn from(err: ProtocolError) -> Self {
        Self {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            reason: err.to_string(),
        }
    }
}

impl From<io::Error> for MyError {
    fn from(err: io::Error) -> Self {
        Self {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            reason: err.to_string(),
        }
    }
}
