use std::sync::Arc;

use blob_kv_engine::Engine;
use blob_kv_protocol::{OpCode, Request, Response, Status};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::Mutex,
};

use crate::error::{RequestHandleError, ToErrorResponse};

pub async fn run(engine: Engine) -> Result<(), ()> {
    let engine = Arc::new(Mutex::new(engine));

    let listener = match TcpListener::bind("0.0.0.0:9999").await {
        Ok(listener) => listener,
        Err(err) => {
            tracing::error!("Failed to bind TcpListener: {:?}", err);
            return Err(());
        }
    };

    loop {
        match listener.accept().await {
            Ok((socket, address)) => {
                let engine = Arc::clone(&engine);
                if let Some(err) =
                    tokio::spawn(async move { handle_connection(socket, engine).await })
                        .await
                        .err()
                {
                    tracing::error!(
                        address = address.to_string(),
                        "Failed to spawn tokio task to handle connection: {:?}",
                        err
                    )
                }
            }
            Err(err) => tracing::error!("Failed to accept connection{:?}", err),
        }
    }
}

async fn handle_connection(mut socket: TcpStream, engine: Arc<Mutex<Engine>>) {
    let request = match read_and_parse(&mut socket).await {
        Ok(request) => request,
        Err(_) => {
            let response = Response::new(Status::InvalidRequest, None).unwrap();
            send_response(socket, response).await;
            return;
        }
    };

    let result = tokio::task::spawn_blocking(async move || {
        let response = match request.op {
            OpCode::Set => handle_set(request, engine).await,
            OpCode::Get => handle_get(request, engine).await,
            OpCode::Delete => handle_del(request, engine).await,
        };

        match response {
            Ok(response) => send_response(socket, response).await,
            Err(err) => {
                tracing::error!("Error handling response: {:?}", err);
                let response = err.to_response();
                send_response(socket, response).await;
            }
        }
    })
    .await;

    if let Some(err) = result.err() {
        tracing::error!(
            "Failed to spawn blocking task to handle connection: {:?}",
            err
        );
        return;
    }
}

async fn read_and_parse(socket: &mut TcpStream) -> Result<Request, ()> {
    let mut buffer = [0u8; 4096];
    let mut data = Vec::new();

    loop {
        let bytes_read = match socket.read(&mut buffer).await {
            Ok(bytes) => bytes,
            Err(err) => {
                tracing::error!("Failed while reading stream {:?}", err);
                return Err(());
            }
        };

        if bytes_read == 0 {
            break;
        }
        data.extend_from_slice(&buffer[..bytes_read]);
    }

    match Request::decode(&data) {
        Ok(request) => Ok(request),
        Err(err) => {
            tracing::error!("Failed to decode response {:?}", err);
            return Err(());
        }
    }
}

type HandleFunctionRetType = Result<Response, RequestHandleError>;

async fn handle_set(request: Request, engine: Arc<Mutex<Engine>>) -> HandleFunctionRetType {
    let mut engine = engine.lock().await;

    match engine.set(&request.key, &request.value.unwrap_or("".to_string())) {
        Ok(_) => Ok(Response::new(Status::Stored, None)?),
        Err(err) => Err(RequestHandleError::from(err)),
    }
}

async fn handle_get(request: Request, engine: Arc<Mutex<Engine>>) -> HandleFunctionRetType {
    let engine = engine.lock().await;

    match engine.get(&request.key) {
        Ok(value) => Ok(Response::new(Status::Ok, value.as_deref())?),
        Err(err) => Err(RequestHandleError::from(err)),
    }
}

async fn handle_del(request: Request, engine: Arc<Mutex<Engine>>) -> HandleFunctionRetType {
    let mut engine = engine.lock().await;

    match engine.del(&request.key) {
        Ok(deleted) => Ok(if deleted {
            Response::new(Status::Deleted, None)?
        } else {
            Response::new(Status::KeyNotFound, None)?
        }),
        Err(err) => Err(RequestHandleError::from(err)),
    }
}

async fn send_response(mut socket: TcpStream, response: Response) {
    match socket.write_all(response.encode().as_ref()).await {
        Err(err) => tracing::error!("Failed to send response: {:?}", err),
        Ok(_) => tracing::debug!("Sent response"),
    }
}
