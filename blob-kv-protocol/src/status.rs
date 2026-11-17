/// Indicates the outcome of a request.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Ok = 0x00,
    Stored = 0x01,
    Deleted = 0x02,
    KeyNotFound = 0x03,
    InvalidRequest = 0x04,
    UnsupportedOperation = 0x05,
    InternalServerError = 0x06,
    ValueTooLarge = 0x07,
    InvalidProtocolVersion = 0x08,
}

impl Status {
    pub fn new(byte: u8) -> Result<Self, StatusError> {
        match byte {
            0x00 => Ok(Self::Ok),
            0x01 => Ok(Self::Stored),
            0x02 => Ok(Self::Deleted),
            0x03 => Ok(Self::KeyNotFound),
            0x04 => Ok(Self::InvalidRequest),
            0x05 => Ok(Self::UnsupportedOperation),
            0x06 => Ok(Self::InternalServerError),
            0x07 => Ok(Self::ValueTooLarge),
            0x08 => Ok(Self::InvalidProtocolVersion),
            _ => Err(StatusError::InvalidStatus(byte)),
        }
    }

    pub fn encode(&self) -> u8 {
        *self as u8
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, StatusError> {
        let byte = match bytes {
            [byte] => *byte,
            _ => return Err(StatusError::InvalidLength(bytes.len())),
        };

        Self::new(byte)
    }

    pub fn is_ok(&self) -> bool {
        matches!(self, Self::Ok | Self::Stored | Self::Deleted)
    }
}

#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
pub enum StatusError {
    #[error("got invalid status '{0}'")]
    InvalidStatus(u8),
    #[error("status must be exactly 1 byte, got '{0}' bytes")]
    InvalidLength(usize),
}

#[cfg(test)]
mod test {
    use super::*;

    const ALL_STATUSES: [(Status, u8); 9] = [
        (Status::Ok, 0x00),
        (Status::Stored, 0x01),
        (Status::Deleted, 0x02),
        (Status::KeyNotFound, 0x03),
        (Status::InvalidRequest, 0x04),
        (Status::UnsupportedOperation, 0x05),
        (Status::InternalServerError, 0x06),
        (Status::ValueTooLarge, 0x07),
        (Status::InvalidProtocolVersion, 0x08),
    ];

    #[test]
    fn it_decodes_all_statuses() {
        for (status, byte) in ALL_STATUSES {
            assert_eq!(Status::decode(&[byte]), Ok(status));
        }
    }

    #[test]
    fn it_encodes_all_statuses() {
        for (status, byte) in ALL_STATUSES {
            assert_eq!(status.encode(), byte);
        }
    }

    #[test]
    fn it_encodes_and_decodes() {
        for (status, _) in ALL_STATUSES {
            assert_eq!(Status::decode(&[status.encode()]), Ok(status));
        }
    }

    #[test]
    fn it_does_not_decode_when_length_is_not_one() {
        assert_eq!(Status::decode(&[]), Err(StatusError::InvalidLength(0)));
        assert_eq!(
            Status::decode(&[0x00, 0x01]),
            Err(StatusError::InvalidLength(2))
        );
    }

    #[test]
    fn it_does_not_decode_invalid_statuses() {
        assert_eq!(
            Status::decode(&[0x09]),
            Err(StatusError::InvalidStatus(0x09))
        );
        assert_eq!(
            Status::decode(&[0xFF]),
            Err(StatusError::InvalidStatus(0xFF))
        );
    }

    #[test]
    fn it_does_not_create_with_invalid_status() {
        assert_eq!(Status::new(0x09), Err(StatusError::InvalidStatus(0x09)));
        assert_eq!(Status::new(0xFF), Err(StatusError::InvalidStatus(0xFF)));
    }

    #[test]
    fn it_reports_whether_status_is_ok() {
        assert!(Status::Ok.is_ok());
        assert!(Status::Stored.is_ok());
        assert!(Status::Deleted.is_ok());
        assert!(!Status::KeyNotFound.is_ok());
        assert!(!Status::InternalServerError.is_ok());
    }
}
