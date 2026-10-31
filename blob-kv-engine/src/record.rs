use crc::CRC_32_ISO_HDLC;
use crc::Crc;

const CRC: Crc<u32> = Crc::<u32>::new(&CRC_32_ISO_HDLC);

const RECORD_HEADER_LENGTH: u32 = 14;
const RECORD_CHECKSUM_LENGTH: u32 = 4;
const RECORD_MIN_LENGTH: u32 = RECORD_HEADER_LENGTH + RECORD_CHECKSUM_LENGTH; // Considering empty key and value bytes
const RECORD_ENCODING_VERSION: u8 = 1;
const RECORD_MAX_LENGTH: u32 = u32::MAX;

/// Byte encode format:
/// * record length (4 bytes)
/// * format version (1 byte)
/// * op type (1 byte)
/// * key length (4 bytes)
/// * value length (4 bytes)
/// * key bytes (variable)
/// * value bytes (variable)
/// * checksum
///
/// The numbers are stored in little-endian format
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    key: String,
    value: String,
    op: RecordOp,
}

impl Record {
    pub fn new(key: &str, value: &str, op: RecordOp) -> Result<Self, RecordCreateError> {
        if key.len() == 0 {
            return Err(RecordCreateError::InvalidKey);
        }

        if RECORD_HEADER_LENGTH
            + key.bytes().len() as u32
            + value.bytes().len() as u32
            + RECORD_CHECKSUM_LENGTH
            >= RECORD_MAX_LENGTH
        {
            return Err(RecordCreateError::SizeLimitExceeded);
        }

        Ok(Self {
            key: key.to_owned(),
            value: value.to_owned(),
            op,
        })
    }

    pub fn encode(&self) -> Vec<u8> {
        let key_bytes = self.key.as_bytes();
        let key_len = key_bytes.len() as u32;

        let value_bytes = self.value.as_bytes();
        let value_len = value_bytes.len() as u32;

        let encoding_len = RECORD_HEADER_LENGTH + key_len + value_len + RECORD_CHECKSUM_LENGTH;
        let mut encoding = Vec::with_capacity(encoding_len as usize);

        encoding.extend_from_slice(&encoding_len.to_le_bytes());
        encoding.push(RECORD_ENCODING_VERSION);
        encoding.push(self.op.clone() as u8);
        encoding.extend_from_slice(&key_len.to_le_bytes());
        encoding.extend_from_slice(&value_len.to_le_bytes());
        encoding.extend_from_slice(&key_bytes);
        encoding.extend_from_slice(&value_bytes);

        let checksum = CRC.checksum(&encoding);
        encoding.extend_from_slice(&checksum.to_le_bytes());

        encoding
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, RecordDecodeError> {
        if (bytes.len() as u32) < RECORD_MIN_LENGTH {
            return Err(RecordDecodeError::InvalidRecordSize);
        }

        unsafe {
            let mut offset = 0;

            // Read the header
            let length =
                u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap_unchecked());
            offset += 4;
            if bytes.len() as u32 != length {
                return Err(RecordDecodeError::LengthMismatch);
            }

            let version =
                u8::from_le_bytes(bytes[offset..offset + 1].try_into().unwrap_unchecked());
            offset += 1;
            if version != RECORD_ENCODING_VERSION {
                return Err(RecordDecodeError::InvalidVersion);
            }

            let op_byte =
                u8::from_le_bytes(bytes[offset..offset + 1].try_into().unwrap_unchecked());
            offset += 1;
            let op = RecordOp::from(op_byte).map_err(|_| RecordDecodeError::InvalidOp)?;

            let key_len =
                u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap_unchecked());
            offset += 4;
            let value_len =
                u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap_unchecked());
            offset += 4;

            if key_len == 0 {
                return Err(RecordDecodeError::InvalidKey);
            }
            if key_len + value_len != length - RECORD_HEADER_LENGTH - 4 {
                return Err(RecordDecodeError::InvalidKeyValueLength);
            }

            // Read the key and value
            let key = String::from_utf8(bytes[offset..offset + key_len as usize].to_vec())
                .map_err(|_| RecordDecodeError::InvalidKeyBytes)?;
            offset += key_len as usize;
            let value = String::from_utf8(bytes[offset..offset + value_len as usize].to_vec())
                .map_err(|_| RecordDecodeError::InvalidValueBytes)?;
            offset += value_len as usize;

            let checksum =
                u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap_unchecked());
            let checksum_expected = CRC.checksum(&bytes[0..offset]);
            if checksum != checksum_expected {
                return Err(RecordDecodeError::ChecksumMismatch);
            }

            Ok(Record::new(&key, &value, op).unwrap_unchecked())
        }
    }
}

#[repr(u8)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecordOp {
    Set = 0,
    Del = 1,
}

impl RecordOp {
    fn from(byte: u8) -> Result<RecordOp, ()> {
        match byte {
            0 => Ok(RecordOp::Set),
            1 => Ok(RecordOp::Del),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecordCreateError {
    InvalidKey,
    SizeLimitExceeded,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecordDecodeError {
    ChecksumMismatch,
    LengthMismatch,
    InvalidKey,
    InvalidKeyValueLength,
    InvalidRecordSize,
    InvalidOp,
    InvalidVersion,
    InvalidKeyBytes,
    InvalidValueBytes,
}

#[cfg(test)]
mod test {
    use super::*;

    /// Encoding for
    /// ```
    /// Record::new("Hello, "World", RecordOp::Set);
    /// ```
    const ENCODING: [u8; 28] = [
        28, 0, 0, 0, 1, 0, 5, 0, 0, 0, 5, 0, 0, 0, 72, 101, 108, 108, 111, 87, 111, 114, 108, 100,
        63, 71, 98, 246,
    ];
    const INVALID_UTF8: [u8; 2] = [0xFF, 0xFE];

    #[test]
    fn it_does_not_create_record_with_empty_key() {
        assert_eq!(
            Record::new("", "World", RecordOp::Set),
            Err(RecordCreateError::InvalidKey)
        );
    }

    #[test]
    fn it_encodes() {
        let record = Record::new("Hello", "World", RecordOp::Set).unwrap();
        assert_eq!(record.encode().as_slice(), ENCODING);
    }

    #[test]
    fn it_encodes_and_decodes() {
        let record = Record::new("Hello", "World", RecordOp::Set).unwrap();
        let decoded_record = Record::decode(&record.encode()).unwrap();
        assert_eq!(decoded_record, record);
    }

    #[test]
    fn it_decodes() {
        let decoded_record = Record::decode(&ENCODING).unwrap();
        let expected_record = Record::new("Hello", "World", RecordOp::Set).unwrap();
        assert_eq!(decoded_record, expected_record);
    }

    #[test]
    fn it_encodes_and_decodes_empty_value() {
        let record = Record::new("Hello", "", RecordOp::Del).unwrap();
        let encoding = record.encode();
        let decoded_record = Record::decode(&encoding);
        assert_eq!(decoded_record, Ok(record));
    }

    #[test]
    fn it_does_not_decode_when_length_is_less_then_min() {
        assert_eq!(
            Record::decode(&[1, 2, 3, 4, 5]),
            Err(RecordDecodeError::InvalidRecordSize)
        );
        assert_eq!(
            Record::decode(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17]),
            Err(RecordDecodeError::InvalidRecordSize)
        );
    }

    #[test]
    fn it_does_not_decode_when_length_is_wrong() {
        let mut encoding = ENCODING;
        encoding[0] = 32; // Modify length to something else
        assert_eq!(
            Record::decode(&encoding),
            Err(RecordDecodeError::LengthMismatch)
        );
    }

    #[test]
    fn it_does_not_decode_when_version_is_invalid() {
        let mut encoding = ENCODING;
        encoding[4] = 99; // Set invalid version
        assert_eq!(
            Record::decode(&encoding),
            Err(RecordDecodeError::InvalidVersion)
        );
    }

    #[test]
    fn it_does_not_decode_when_op_is_invalid() {
        let mut encoding = ENCODING;
        encoding[5] = 99; // Set invalid op
        assert_eq!(Record::decode(&encoding), Err(RecordDecodeError::InvalidOp));
    }

    #[test]
    fn it_does_not_decode_when_key_is_empty() {
        let mut encoding = ENCODING;
        // Set key length to 0
        encoding[6] = 0;
        encoding[7] = 0;
        encoding[8] = 0;
        encoding[9] = 0;
        assert_eq!(
            Record::decode(&encoding),
            Err(RecordDecodeError::InvalidKey)
        );
    }

    #[test]
    fn it_does_not_decode_when_key_and_value_length_values_are_less_then_expected() {
        let mut encoding = ENCODING;
        encoding[6] -= 1;
        encoding[10] -= 1;
        assert_eq!(
            Record::decode(&encoding),
            Err(RecordDecodeError::InvalidKeyValueLength)
        );
    }

    #[test]
    fn it_does_not_decode_when_key_and_value_length_values_are_more_then_expected() {
        let mut encoding = ENCODING;
        // Set key length to
        encoding[6] += 1;
        encoding[10] += 1;
        assert_eq!(
            Record::decode(&encoding),
            Err(RecordDecodeError::InvalidKeyValueLength)
        );
    }

    #[test]
    fn it_does_not_decode_when_key_is_invalid_utf8_bytes() {
        let mut encoding = ENCODING;
        // Make first 2 bytes of the key invalid UTF8
        encoding[14] = INVALID_UTF8[0];
        encoding[15] = INVALID_UTF8[1];
        assert_eq!(
            Record::decode(&encoding),
            Err(RecordDecodeError::InvalidKeyBytes)
        );
    }

    #[test]
    fn it_does_not_decode_when_value_is_invalid_utf8_bytes() {
        let mut encoding = ENCODING;
        // Make first 2 bytes of the value invalid UTF8
        encoding[19] = INVALID_UTF8[0];
        encoding[20] = INVALID_UTF8[1];
        assert_eq!(
            Record::decode(&encoding),
            Err(RecordDecodeError::InvalidValueBytes)
        );
    }

    #[test]
    fn it_does_not_decode_when_checksum_is_wrong() {
        let mut encoding = ENCODING;
        // Modify checksum to something else
        encoding[27] = 0;
        assert_eq!(
            Record::decode(&encoding),
            Err(RecordDecodeError::ChecksumMismatch)
        );
    }
}
