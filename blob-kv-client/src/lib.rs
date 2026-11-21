use std::{
    io::{self, Read, Write},
    net::TcpStream,
};

use blob_kv_protocol::{
    OpCode, RESPONSE_HEADER_LENGTH, Request, RequestCreateError, ResponseDecodeError,
};

pub use blob_kv_protocol::Response;

pub struct Client {
    stream: TcpStream,
}

impl Client {
    pub fn connect(addr: &'static str) -> Result<Self, Error> {
        let stream = TcpStream::connect(addr).map_err(|source| Error::Connect { addr, source })?;
        Ok(Self { stream })
    }

    pub fn get(&mut self, key: &str) -> Result<Response, Error> {
        self.request(OpCode::Get, key, None)
    }

    pub fn set(&mut self, key: &str, value: &str) -> Result<Response, Error> {
        self.request(OpCode::Set, key, Some(value))
    }

    pub fn del(&mut self, key: &str) -> Result<Response, Error> {
        self.request(OpCode::Delete, key, None)
    }

    fn request(&mut self, op: OpCode, key: &str, value: Option<&str>) -> Result<Response, Error> {
        let request = Request::new(op, key, value)?;
        self.send(&request)
    }

    fn send(&mut self, request: &Request) -> Result<Response, Error> {
        self.stream.write_all(&request.encode())?;
        self.stream.flush()?;

        let header_len = RESPONSE_HEADER_LENGTH as usize;
        let mut header = [0u8; RESPONSE_HEADER_LENGTH as usize];
        self.stream.read_exact(&mut header)?;

        let payload_len = u32::from_be_bytes(
            header[2..6]
                .try_into()
                .expect("response payload length is 4 bytes"),
        ) as usize;

        let mut frame = vec![0u8; header_len + payload_len];
        frame[..header_len].copy_from_slice(&header);
        self.stream.read_exact(&mut frame[header_len..])?;

        Ok(Response::decode(&frame)?)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to connect to server at {addr}: {source}")]
    Connect {
        addr: &'static str,
        #[source]
        source: io::Error,
    },
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Request(#[from] RequestCreateError),
    #[error(transparent)]
    Response(#[from] ResponseDecodeError),
}
