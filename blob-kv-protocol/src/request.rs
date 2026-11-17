use std::string::FromUtf8Error;

use crate::opcode::{OpCode, OpCodeError};

const REQUEST_HEADER_LENGTH: u32 = 5; // version (1 byte) + payload length (4 bytes)
const REQUEST_OPCODE_LENGTH: u32 = 1;
const REQUEST_KEY_LENGTH_LENGTH: u32 = 4;
const REQUEST_VALUE_LENGTH_LENGTH: u32 = 4;
const REQUEST_GET_DELETE_FIXED_PAYLOAD_LENGTH: u32 =
    REQUEST_OPCODE_LENGTH + REQUEST_KEY_LENGTH_LENGTH; // Considering empty key
const REQUEST_MIN_LENGTH: u32 = REQUEST_HEADER_LENGTH + REQUEST_GET_DELETE_FIXED_PAYLOAD_LENGTH + 1; // Considering one key byte
const REQUEST_ENCODING_VERSION: u8 = 1;
const REQUEST_MAX_LENGTH: u32 = u32::MAX;

/// Byte encode format:
/// * format version (1 byte)
/// * payload length (4 bytes)
/// * opcode (1 byte)
/// * key length (4 bytes)
/// * key bytes (variable)
/// * value length (4 bytes, only for `SET`)
/// * value bytes (variable, only for `SET`)
///
/// The numbers are stored in big-endian format. The payload length covers
/// every field that follows it. The value is only encoded for `SET` requests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub op: OpCode,
    pub key: String,
    pub value: Option<String>,
}

impl Request {
    /// Creates a request.
    ///
    /// `value` must be present for [`OpCode::Set`] and absent for every other
    /// operation.
    pub fn new(op: OpCode, key: &str, value: Option<&str>) -> Result<Self, RequestCreateError> {
        tracing::debug!(operation=?op, key, "Request create initiated");

        if key.is_empty() {
            return Err(RequestCreateError::InvalidKey(key.to_string()));
        }

        let value = match (op.takes_value(), value) {
            (true, Some(value)) => Some(value.to_owned()),
            (true, None) => return Err(RequestCreateError::MissingValue),
            (false, Some(_)) => return Err(RequestCreateError::UnexpectedValue),
            (false, None) => None,
        };

        let key_len = key.len() as u32;
        let len = REQUEST_HEADER_LENGTH
            + REQUEST_GET_DELETE_FIXED_PAYLOAD_LENGTH
            + key_len
            + match &value {
                Some(value) => REQUEST_VALUE_LENGTH_LENGTH + value.len() as u32,
                None => 0,
            };

        if len == REQUEST_MAX_LENGTH {
            return Err(RequestCreateError::SizeLimitExceeded {
                len,
                max_len: REQUEST_MAX_LENGTH,
            });
        }

        tracing::debug!(operation=?op, key, "Request create finished");
        Ok(Self {
            op,
            key: key.to_owned(),
            value,
        })
    }

    pub fn encode(&self) -> Vec<u8> {
        let key_bytes = self.key.as_bytes();
        let key_len = key_bytes.len() as u32;

        let value_bytes = if self.op.takes_value() {
            self.value.as_deref().unwrap_or_default().as_bytes()
        } else {
            &[]
        };
        let value_len = value_bytes.len() as u32;

        let mut payload_len = REQUEST_GET_DELETE_FIXED_PAYLOAD_LENGTH + key_len;
        if self.op.takes_value() {
            payload_len += REQUEST_VALUE_LENGTH_LENGTH + value_len;
        }

        let mut encoding = Vec::with_capacity((REQUEST_HEADER_LENGTH + payload_len) as usize);

        encoding.push(REQUEST_ENCODING_VERSION);
        encoding.extend_from_slice(&payload_len.to_be_bytes());
        encoding.push(self.op.encode());
        encoding.extend_from_slice(&key_len.to_be_bytes());
        encoding.extend_from_slice(key_bytes);

        if self.op.takes_value() {
            encoding.extend_from_slice(&value_len.to_be_bytes());
            encoding.extend_from_slice(value_bytes);
        }

        encoding
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, RequestDecodeError> {
        let request_len = bytes.len() as u32;
        if request_len < REQUEST_MIN_LENGTH {
            return Err(RequestDecodeError::RequestSizeTooLow(request_len));
        }

        if request_len == REQUEST_MAX_LENGTH {
            return Err(RequestDecodeError::RequestSizeTooHigh(request_len));
        }

        // SAFETY: the minimum request length check above guarantees that every
        // fixed-size header field can be read. Variable-length fields are
        // bounds-checked before they are sliced.
        unsafe {
            let mut offset = 0;

            let version = bytes[offset];
            offset += 1;
            if version != REQUEST_ENCODING_VERSION {
                return Err(RequestDecodeError::InvalidVersion(version));
            }

            let payload_len =
                u32::from_be_bytes(bytes[offset..offset + 4].try_into().unwrap_unchecked());
            offset += 4;

            let actual_payload_len = request_len - REQUEST_HEADER_LENGTH;
            if payload_len != actual_payload_len {
                return Err(RequestDecodeError::PayloadLengthMismatch {
                    payload_len,
                    expected: actual_payload_len,
                });
            }

            let op_byte = bytes[offset];
            offset += 1;
            let op = OpCode::new(op_byte)?;

            // Read the key
            let key_len =
                u32::from_be_bytes(bytes[offset..offset + 4].try_into().unwrap_unchecked());
            offset += 4;

            if key_len == 0 {
                return Err(RequestDecodeError::InvalidKey);
            }

            let key_end = offset + key_len as usize;
            if key_end > bytes.len() {
                return Err(RequestDecodeError::UnexpectedEnd {
                    needed: key_end,
                    available: bytes.len(),
                });
            }

            let key = String::from_utf8(bytes[offset..key_end].to_vec())?;
            offset = key_end;

            let value = if op.takes_value() {
                let value_len_end = offset + REQUEST_VALUE_LENGTH_LENGTH as usize;
                if value_len_end > bytes.len() {
                    return Err(RequestDecodeError::UnexpectedEnd {
                        needed: value_len_end,
                        available: bytes.len(),
                    });
                }

                let value_len =
                    u32::from_be_bytes(bytes[offset..value_len_end].try_into().unwrap_unchecked());
                offset = value_len_end;

                let value_end = offset + value_len as usize;
                if value_end > bytes.len() {
                    return Err(RequestDecodeError::UnexpectedEnd {
                        needed: value_end,
                        available: bytes.len(),
                    });
                }

                let value = String::from_utf8(bytes[offset..value_end].to_vec())?;
                offset = value_end;

                Some(value)
            } else {
                None
            };

            if offset != bytes.len() {
                return Err(RequestDecodeError::TrailingBytes {
                    extra: bytes.len() - offset,
                });
            }

            tracing::debug!(operation=?op, key, "Request decode finished");

            Ok(Self { op, key, value })
        }
    }
}

#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
pub enum RequestCreateError {
    #[error("invalid key '{0}'")]
    InvalidKey(String),
    #[error("SET requests require a value")]
    MissingValue,
    #[error("only SET requests can carry a value")]
    UnexpectedValue,
    #[error("size limit exceeded, got '{len:?}' bytes, max allowed bytes '{max_len:?}'")]
    SizeLimitExceeded { len: u32, max_len: u32 },
}

#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
pub enum RequestDecodeError {
    #[error("payload length mismatch, got '{payload_len:?}', expected '{expected:?}'")]
    PayloadLengthMismatch { payload_len: u32, expected: u32 },
    #[error("request size below required minimum of {REQUEST_MIN_LENGTH}, got '{0}'")]
    RequestSizeTooLow(u32),
    #[error("request size above allowed maximum of {REQUEST_MAX_LENGTH}, got '{0}'")]
    RequestSizeTooHigh(u32),
    #[error("got invalid version '{0}'")]
    InvalidVersion(u8),
    #[error(transparent)]
    InvalidOp(#[from] OpCodeError),
    #[error("invalid key")]
    InvalidKey,
    #[error("unexpected end of request, needed at least '{needed}' bytes, got '{available}'")]
    UnexpectedEnd { needed: usize, available: usize },
    #[error("request has '{extra}' trailing bytes")]
    TrailingBytes { extra: usize },
    #[error("invalid UTF-8 bytes: '{0}'")]
    InvalidUtf8(#[from] FromUtf8Error),
}

#[cfg(test)]
mod test {
    use std::assert_matches;

    use super::*;

    /// Encoding for
    /// ```
    /// Request::new(OpCode::Set, "Hello", Some("World"));
    /// ```
    const SET_ENCODING: [u8; 24] = [
        1, // version
        0, 0, 0, 19, // payload length
        1,  // opcode SET
        0, 0, 0, 5, // key length
        72, 101, 108, 108, 111, // "Hello"
        0, 0, 0, 5, // value length
        87, 111, 114, 108, 100, // "World"
    ];

    /// Encoding for
    /// ```
    /// Request::new(OpCode::Get, "Hello", None);
    /// ```
    const GET_ENCODING: [u8; 15] = [
        1, // version
        0, 0, 0, 10, // payload length
        2,  // opcode GET
        0, 0, 0, 5, // key length
        72, 101, 108, 108, 111, // "Hello"
    ];

    const INVALID_UTF8: [u8; 2] = [0xFF, 0xFE];

    #[test]
    fn it_does_not_create_request_with_empty_key() {
        assert_eq!(
            Request::new(OpCode::Set, "", Some("World")),
            Err(RequestCreateError::InvalidKey("".to_string()))
        );
        assert_eq!(
            Request::new(OpCode::Get, "", None),
            Err(RequestCreateError::InvalidKey("".to_string()))
        );
    }

    #[test]
    fn it_does_not_create_set_request_without_value() {
        assert_eq!(
            Request::new(OpCode::Set, "Hello", None),
            Err(RequestCreateError::MissingValue)
        );
    }

    #[test]
    fn it_does_not_create_request_with_unexpected_value() {
        assert_eq!(
            Request::new(OpCode::Get, "Hello", Some("World")),
            Err(RequestCreateError::UnexpectedValue)
        );
        assert_eq!(
            Request::new(OpCode::Delete, "Hello", Some("World")),
            Err(RequestCreateError::UnexpectedValue)
        );
    }

    #[test]
    fn it_encodes_set() {
        let request = Request::new(OpCode::Set, "Hello", Some("World")).unwrap();
        assert_eq!(request.encode().as_slice(), SET_ENCODING);
    }

    #[test]
    fn it_encodes_get() {
        let request = Request::new(OpCode::Get, "Hello", None).unwrap();
        assert_eq!(request.encode().as_slice(), GET_ENCODING);
    }

    #[test]
    fn it_encodes_and_decodes_set() {
        let request = Request::new(OpCode::Set, "Hello", Some("World")).unwrap();
        let decoded_request = Request::decode(&request.encode()).unwrap();
        assert_eq!(decoded_request, request);
    }

    #[test]
    fn it_encodes_and_decodes_get() {
        let request = Request::new(OpCode::Get, "Hello", None).unwrap();
        let decoded_request = Request::decode(&request.encode()).unwrap();
        assert_eq!(decoded_request, request);
    }

    #[test]
    fn it_encodes_and_decodes_delete() {
        let request = Request::new(OpCode::Delete, "Hello", None).unwrap();
        let decoded_request = Request::decode(&request.encode()).unwrap();
        assert_eq!(decoded_request, request);
    }

    #[test]
    fn it_encodes_and_decodes_empty_value() {
        let request = Request::new(OpCode::Set, "Hello", Some("")).unwrap();
        let encoding = request.encode();
        let decoded_request = Request::decode(&encoding);
        assert_eq!(decoded_request, Ok(request));
    }

    #[test]
    fn it_decodes_set() {
        let decoded_request = Request::decode(&SET_ENCODING).unwrap();
        let expected_request = Request::new(OpCode::Set, "Hello", Some("World")).unwrap();
        assert_eq!(decoded_request, expected_request);
    }

    #[test]
    fn it_decodes_get() {
        let decoded_request = Request::decode(&GET_ENCODING).unwrap();
        let expected_request = Request::new(OpCode::Get, "Hello", None).unwrap();
        assert_eq!(decoded_request, expected_request);
    }

    #[test]
    fn it_does_not_decode_when_length_is_less_than_min() {
        assert_eq!(
            Request::decode(&[1, 2, 3, 4, 5]),
            Err(RequestDecodeError::RequestSizeTooLow(5))
        );
        assert_eq!(
            Request::decode(&[0; 10]),
            Err(RequestDecodeError::RequestSizeTooLow(10))
        );
    }

    #[test]
    fn it_does_not_decode_when_payload_length_is_wrong() {
        let mut encoding = SET_ENCODING;
        encoding[1..5].copy_from_slice(&24u32.to_be_bytes()); // Modify payload length to something else
        assert_eq!(
            Request::decode(&encoding),
            Err(RequestDecodeError::PayloadLengthMismatch {
                payload_len: 24,
                expected: SET_ENCODING.len() as u32 - REQUEST_HEADER_LENGTH,
            })
        );
    }

    #[test]
    fn it_does_not_decode_when_version_is_invalid() {
        let mut encoding = SET_ENCODING;
        encoding[0] = 99; // Set invalid version
        assert_eq!(
            Request::decode(&encoding),
            Err(RequestDecodeError::InvalidVersion(99))
        );
    }

    #[test]
    fn it_does_not_decode_when_opcode_is_invalid() {
        let mut encoding = SET_ENCODING;
        encoding[5] = 99; // Set invalid opcode
        assert_eq!(
            Request::decode(&encoding),
            Err(RequestDecodeError::InvalidOp(OpCodeError::InvalidOpCode(
                99
            )))
        );
    }

    #[test]
    fn it_does_not_decode_when_key_is_empty() {
        let mut encoding = SET_ENCODING;
        encoding[6..10].copy_from_slice(&0u32.to_be_bytes());
        assert_eq!(
            Request::decode(&encoding),
            Err(RequestDecodeError::InvalidKey)
        );
    }

    #[test]
    fn it_does_not_decode_when_value_length_is_missing() {
        // A SET request without room for the value length field.
        let encoding: [u8; 11] = [
            1, // version
            0, 0, 0, 6, // payload length
            1, // opcode SET
            0, 0, 0, 1,  // key length
            65, // "A"
        ];
        assert_eq!(
            Request::decode(&encoding),
            Err(RequestDecodeError::UnexpectedEnd {
                needed: 15,
                available: encoding.len(),
            })
        );
    }

    #[test]
    fn it_does_not_decode_when_key_is_truncated() {
        let mut encoding = GET_ENCODING;
        encoding[6..10].copy_from_slice(&6u32.to_be_bytes());
        assert_eq!(
            Request::decode(&encoding),
            Err(RequestDecodeError::UnexpectedEnd {
                needed: 16,
                available: GET_ENCODING.len(),
            })
        );
    }

    #[test]
    fn it_does_not_decode_when_value_is_truncated() {
        let mut encoding = SET_ENCODING;
        encoding[15..19].copy_from_slice(&6u32.to_be_bytes());
        assert_eq!(
            Request::decode(&encoding),
            Err(RequestDecodeError::UnexpectedEnd {
                needed: 25,
                available: SET_ENCODING.len(),
            })
        );
    }

    #[test]
    fn it_does_not_decode_set_when_there_are_trailing_bytes() {
        let mut encoding = SET_ENCODING;
        encoding[15..19].copy_from_slice(&4u32.to_be_bytes());
        assert_eq!(
            Request::decode(&encoding),
            Err(RequestDecodeError::TrailingBytes { extra: 1 })
        );
    }

    #[test]
    fn it_does_not_decode_get_when_there_are_trailing_bytes() {
        let mut encoding = GET_ENCODING;
        encoding[6..10].copy_from_slice(&4u32.to_be_bytes());
        assert_eq!(
            Request::decode(&encoding),
            Err(RequestDecodeError::TrailingBytes { extra: 1 })
        );
    }

    #[test]
    fn it_does_not_decode_when_key_is_invalid_utf8_bytes() {
        let mut encoding = SET_ENCODING;
        encoding[10] = INVALID_UTF8[0];
        encoding[11] = INVALID_UTF8[1];
        assert_matches!(
            Request::decode(&encoding),
            Err(RequestDecodeError::InvalidUtf8(_))
        );
    }

    #[test]
    fn it_does_not_decode_when_value_is_invalid_utf8_bytes() {
        let mut encoding = SET_ENCODING;
        encoding[19] = INVALID_UTF8[0];
        encoding[20] = INVALID_UTF8[1];
        assert_matches!(
            Request::decode(&encoding),
            Err(RequestDecodeError::InvalidUtf8(_))
        );
    }
}
