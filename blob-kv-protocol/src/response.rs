use std::string::FromUtf8Error;

use crate::status::{Status, StatusError};

const RESPONSE_HEADER_LENGTH: u32 = 6; // version (1 byte) + status (1 byte) + payload length (4 bytes)
const RESPONSE_ENCODING_VERSION: u8 = 1;
const RESPONSE_MAX_LENGTH: u32 = u32::MAX;

/// Byte encode format:
/// * format version (1 byte)
/// * status (1 byte)
/// * payload length (4 bytes)
/// * payload bytes (variable)
///
/// The numbers are stored in big-endian format. The payload length covers
/// the bytes that follow the header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
    pub status: Status,
    pub payload: String,
}

impl Response {
    pub fn new(status: Status, payload: &str) -> Result<Self, ResponseCreateError> {
        tracing::debug!(status=?status, payload, "Response create initiated");

        let len = RESPONSE_HEADER_LENGTH + payload.len() as u32;
        if len == RESPONSE_MAX_LENGTH {
            return Err(ResponseCreateError::SizeLimitExceeded {
                len,
                max_len: RESPONSE_MAX_LENGTH,
            });
        }

        tracing::debug!(status=?status, "Response create finished");
        Ok(Self {
            status,
            payload: payload.to_owned(),
        })
    }

    pub fn encode(&self) -> Vec<u8> {
        let payload_bytes = self.payload.as_bytes();
        let payload_len = payload_bytes.len() as u32;

        let mut encoding = Vec::with_capacity((RESPONSE_HEADER_LENGTH + payload_len) as usize);

        encoding.push(RESPONSE_ENCODING_VERSION);
        encoding.push(self.status.encode());
        encoding.extend_from_slice(&payload_len.to_be_bytes());
        encoding.extend_from_slice(payload_bytes);

        encoding
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, ResponseDecodeError> {
        let response_len = bytes.len() as u32;
        if response_len < RESPONSE_HEADER_LENGTH {
            return Err(ResponseDecodeError::ResponseSizeTooLow(response_len));
        }

        if response_len == RESPONSE_MAX_LENGTH {
            return Err(ResponseDecodeError::ResponseSizeTooHigh(response_len));
        }

        // SAFETY: the minimum response length check above guarantees that every
        // fixed-size header field can be read. The payload is bounds-checked
        // before it is sliced.
        unsafe {
            let mut offset = 0;

            // Read the header
            let version = bytes[offset];
            offset += 1;
            if version != RESPONSE_ENCODING_VERSION {
                return Err(ResponseDecodeError::InvalidVersion(version));
            }

            let status_byte = bytes[offset];
            offset += 1;
            let status = Status::new(status_byte)?;

            let payload_len =
                u32::from_be_bytes(bytes[offset..offset + 4].try_into().unwrap_unchecked());
            offset += 4;

            // Read the payload
            let payload_end = offset + payload_len as usize;
            if payload_end > bytes.len() {
                return Err(ResponseDecodeError::UnexpectedEnd {
                    needed: payload_end,
                    available: bytes.len(),
                });
            }

            let payload = String::from_utf8(bytes[offset..payload_end].to_vec())?;

            if payload_end != bytes.len() {
                return Err(ResponseDecodeError::TrailingBytes {
                    extra: bytes.len() - payload_end,
                });
            }

            tracing::debug!(status=?status, "Response decode finished");

            Ok(Self { status, payload })
        }
    }
}

#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
pub enum ResponseCreateError {
    #[error("size limit exceeded, got '{len:?}' bytes, max allowed bytes '{max_len:?}'")]
    SizeLimitExceeded { len: u32, max_len: u32 },
}

#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
pub enum ResponseDecodeError {
    #[error("response size below required minimum of {RESPONSE_HEADER_LENGTH}, got '{0}'")]
    ResponseSizeTooLow(u32),
    #[error("response size above allowed maximum of {RESPONSE_MAX_LENGTH}, got '{0}'")]
    ResponseSizeTooHigh(u32),
    #[error("got invalid version '{0}'")]
    InvalidVersion(u8),
    #[error(transparent)]
    InvalidStatus(#[from] StatusError),
    #[error("unexpected end of response, needed at least '{needed}' bytes, got '{available}'")]
    UnexpectedEnd { needed: usize, available: usize },
    #[error("response has '{extra}' trailing bytes")]
    TrailingBytes { extra: usize },
    #[error("invalid payload bytes: '{0}'")]
    InvalidPayloadBytes(#[from] FromUtf8Error),
}

#[cfg(test)]
mod test {
    use std::assert_matches;

    use super::*;

    /// Encoding for
    /// ```
    /// Response::new(Status::Ok, "World");
    /// ```
    const ENCODING: [u8; 11] = [
        1, // version
        0, // status OK
        0, 0, 0, 5, // payload length
        87, 111, 114, 108, 100, // "World"
    ];

    const INVALID_UTF8: [u8; 2] = [0xFF, 0xFE];

    #[test]
    fn it_encodes() {
        let response = Response::new(Status::Ok, "World").unwrap();
        assert_eq!(response.encode().as_slice(), ENCODING);
    }

    #[test]
    fn it_encodes_and_decodes() {
        let response = Response::new(Status::Ok, "World").unwrap();
        let decoded_response = Response::decode(&response.encode()).unwrap();
        assert_eq!(decoded_response, response);
    }

    #[test]
    fn it_decodes() {
        let decoded_response = Response::decode(&ENCODING).unwrap();
        let expected_response = Response::new(Status::Ok, "World").unwrap();
        assert_eq!(decoded_response, expected_response);
    }

    #[test]
    fn it_encodes_and_decodes_empty_payload() {
        let response = Response::new(Status::Ok, "").unwrap();
        let encoding = response.encode();
        assert_eq!(encoding, [1, 0, 0, 0, 0, 0]);
        assert_eq!(Response::decode(&encoding), Ok(response));
    }

    #[test]
    fn it_encodes_and_decodes_error_status_with_payload() {
        let response = Response::new(Status::InternalServerError, "boom").unwrap();
        let decoded_response = Response::decode(&response.encode()).unwrap();
        assert_eq!(decoded_response, response);
    }

    #[test]
    fn it_does_not_decode_when_length_is_less_than_min() {
        assert_eq!(
            Response::decode(&[1, 2, 3, 4, 5]),
            Err(ResponseDecodeError::ResponseSizeTooLow(5))
        );
    }

    #[test]
    fn it_does_not_decode_when_payload_is_truncated() {
        let mut encoding = ENCODING;
        encoding[2..6].copy_from_slice(&6u32.to_be_bytes());
        assert_eq!(
            Response::decode(&encoding),
            Err(ResponseDecodeError::UnexpectedEnd {
                needed: 12,
                available: ENCODING.len(),
            })
        );
    }

    #[test]
    fn it_does_not_decode_when_there_are_trailing_bytes() {
        let mut encoding = ENCODING;
        encoding[2..6].copy_from_slice(&4u32.to_be_bytes());
        assert_eq!(
            Response::decode(&encoding),
            Err(ResponseDecodeError::TrailingBytes { extra: 1 })
        );
    }

    #[test]
    fn it_does_not_decode_when_version_is_invalid() {
        let mut encoding = ENCODING;
        encoding[0] = 99; // Set invalid version
        assert_eq!(
            Response::decode(&encoding),
            Err(ResponseDecodeError::InvalidVersion(99))
        );
    }

    #[test]
    fn it_does_not_decode_when_status_is_invalid() {
        let mut encoding = ENCODING;
        encoding[1] = 99; // Set invalid status
        assert_eq!(
            Response::decode(&encoding),
            Err(ResponseDecodeError::InvalidStatus(
                StatusError::InvalidStatus(99)
            ))
        );
    }

    #[test]
    fn it_does_not_decode_when_payload_is_invalid_utf8_bytes() {
        let mut encoding = ENCODING;
        encoding[6] = INVALID_UTF8[0];
        encoding[7] = INVALID_UTF8[1];
        assert_matches!(
            Response::decode(&encoding),
            Err(ResponseDecodeError::InvalidPayloadBytes(_))
        );
    }
}
