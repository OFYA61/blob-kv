use std::{io::ErrorKind, net::SocketAddr, sync::Arc};

use blob_kv_engine::Engine;
use blob_kv_protocol::{OpCode, REQUEST_HEADER_LENGTH, Request, Response, Status};
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
                tokio::spawn(async move { handle_connection(socket, engine, address).await });
            }
            Err(err) => tracing::error!("Failed to accept connection{:?}", err),
        }
    }
}

async fn handle_connection(mut socket: TcpStream, engine: Arc<Mutex<Engine>>, address: SocketAddr) {
    tracing::info!(address = address.to_string(), "Connection accepted");

    loop {
        let request = match read_and_parse(&mut socket).await {
            Ok(Some(request)) => request,
            Ok(None) => break,
            Err(_) => {
                let response = Response::new(Status::InvalidRequest, None).unwrap();
                let _ = send_response(&mut socket, response).await;
                break;
            }
        };

        let response = match request.op {
            OpCode::Set => handle_set(request, Arc::clone(&engine)).await,
            OpCode::Get => handle_get(request, Arc::clone(&engine)).await,
            OpCode::Delete => handle_del(request, Arc::clone(&engine)).await,
        };

        let response = match response {
            Ok(response) => response,
            Err(err) => {
                tracing::error!("Error handling response: {:?}", err);
                err.to_response()
            }
        };

        if send_response(&mut socket, response).await.is_err() {
            break;
        }
    }

    tracing::info!(address = address.to_string(), "Connection closed");
}

async fn read_and_parse(socket: &mut TcpStream) -> Result<Option<Request>, ()> {
    let header_len = REQUEST_HEADER_LENGTH as usize;
    let mut header = [0u8; REQUEST_HEADER_LENGTH as usize];

    match socket.read_exact(&mut header).await {
        Ok(_) => {}
        Err(err) if err.kind() == ErrorKind::UnexpectedEof => return Ok(None),
        Err(err) => {
            tracing::error!("Failed while reading request header: {:?}", err);
            return Err(());
        }
    }

    // Request header layout: version (1 byte), payload length (4 bytes),
    // reserved (11 bytes).
    let payload_len = u32::from_be_bytes(
        header[1..5]
            .try_into()
            .expect("request payload length is 4 bytes"),
    ) as usize;

    let mut data = vec![0u8; header_len + payload_len];
    data[..header_len].copy_from_slice(&header);

    if let Err(err) = socket.read_exact(&mut data[header_len..]).await {
        tracing::error!("Failed while reading request payload: {:?}", err);
        return Err(());
    }

    match Request::decode(&data) {
        Ok(request) => Ok(Some(request)),
        Err(err) => {
            tracing::error!("Failed to decode request: {:?}", err);
            Err(())
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
        Ok(value) => match value {
            Some(value) => Ok(Response::new(Status::Ok, Some(value.as_str()))?),
            None => Ok(Response::new(Status::KeyNotFound, None)?),
        },
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

async fn send_response(socket: &mut TcpStream, response: Response) -> Result<(), ()> {
    match socket.write_all(response.encode().as_ref()).await {
        Err(err) => {
            tracing::error!("Failed to send response: {:?}", err);
            Err(())
        }
        Ok(_) => {
            tracing::debug!("Sent response");
            Ok(())
        }
    }
}
