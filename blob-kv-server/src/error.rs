use blob_kv_engine::{EngineDelError, EngineGetError, EngineSetError};
use blob_kv_protocol::{Response, ResponseCreateError, Status};

pub trait ToErrorResponse {
    fn to_response(&self) -> Response;
}

#[derive(thiserror::Error, Debug)]
pub enum RequestHandleError {
    #[error(transparent)]
    ResponseCreation(#[from] ResponseCreateError),
    #[error(transparent)]
    EngineGetError(#[from] EngineGetError),
    #[error(transparent)]
    EngineSetError(#[from] EngineSetError),
    #[error(transparent)]
    EngingeDelError(#[from] EngineDelError),
}

impl ToErrorResponse for ResponseCreateError {
    fn to_response(&self) -> Response {
        match self {
            ResponseCreateError::SizeLimitExceeded { len, max_len } => {
                let message = format!("size limit exceeded: {len} bytes (max {max_len})");
                error_response(Status::ValueTooLarge, &message)
            }
        }
    }
}

impl ToErrorResponse for EngineGetError {
    fn to_response(&self) -> Response {
        match self {
            EngineGetError::LogScan(_) => {
                error_response(Status::InternalServerError, "failed to scan log")
            }
        }
    }
}

impl ToErrorResponse for EngineSetError {
    fn to_response(&self) -> Response {
        match self {
            EngineSetError::LogAppend(_) => {
                error_response(Status::InternalServerError, "failed to append log")
            }
            EngineSetError::RecordCreate(err) => {
                error_response(Status::ValueTooLarge, &err.to_string())
            }
        }
    }
}

impl ToErrorResponse for EngineDelError {
    fn to_response(&self) -> Response {
        match self {
            EngineDelError::LogAppend(_) => {
                error_response(Status::InternalServerError, "failed to append log")
            }
            EngineDelError::RecordCreate(err) => {
                error_response(Status::ValueTooLarge, &err.to_string())
            }
        }
    }
}

impl ToErrorResponse for RequestHandleError {
    fn to_response(&self) -> Response {
        match self {
            RequestHandleError::ResponseCreation(err) => err.to_response(),
            RequestHandleError::EngineGetError(err) => err.to_response(),
            RequestHandleError::EngineSetError(err) => err.to_response(),
            RequestHandleError::EngingeDelError(err) => err.to_response(),
        }
    }
}

fn error_response(status: Status, message: &str) -> Response {
    Response::new(status, Some(message)).expect("error response payload is within the size limit")
}
