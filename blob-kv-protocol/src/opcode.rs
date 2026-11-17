/// The operation that a request is performing.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpCode {
    Set = 1,
    Get = 2,
    Delete = 3,
}

impl OpCode {
    pub fn new(byte: u8) -> Result<Self, OpCodeError> {
        match byte {
            1 => Ok(Self::Set),
            2 => Ok(Self::Get),
            3 => Ok(Self::Delete),
            _ => Err(OpCodeError::InvalidOpCode(byte)),
        }
    }

    pub fn encode(&self) -> u8 {
        *self as u8
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, OpCodeError> {
        let byte = match bytes {
            [byte] => *byte,
            _ => return Err(OpCodeError::InvalidLength(bytes.len())),
        };

        Self::new(byte)
    }

    pub fn takes_value(&self) -> bool {
        matches!(self, Self::Set)
    }
}

#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
pub enum OpCodeError {
    #[error("got invalid opcode '{0}'")]
    InvalidOpCode(u8),
    #[error("opcode must be exactly 1 byte, got '{0}' bytes")]
    InvalidLength(usize),
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn it_decodes_all_opcodes() {
        assert_eq!(OpCode::decode(&[1]), Ok(OpCode::Set));
        assert_eq!(OpCode::decode(&[2]), Ok(OpCode::Get));
        assert_eq!(OpCode::decode(&[3]), Ok(OpCode::Delete));
    }

    #[test]
    fn it_encodes_all_opcodes() {
        assert_eq!(OpCode::Set.encode(), 1);
        assert_eq!(OpCode::Get.encode(), 2);
        assert_eq!(OpCode::Delete.encode(), 3);
    }

    #[test]
    fn it_encodes_and_decodes() {
        for op in [OpCode::Set, OpCode::Get, OpCode::Delete] {
            assert_eq!(OpCode::decode(&[op.encode()]), Ok(op));
        }
    }

    #[test]
    fn it_does_not_decode_when_length_is_not_one() {
        assert_eq!(OpCode::decode(&[]), Err(OpCodeError::InvalidLength(0)));
        assert_eq!(OpCode::decode(&[1, 2]), Err(OpCodeError::InvalidLength(2)));
    }

    #[test]
    fn it_does_not_decode_invalid_opcodes() {
        assert_eq!(OpCode::decode(&[0]), Err(OpCodeError::InvalidOpCode(0)));
        assert_eq!(OpCode::decode(&[4]), Err(OpCodeError::InvalidOpCode(4)));
        assert_eq!(OpCode::decode(&[99]), Err(OpCodeError::InvalidOpCode(99)));
    }

    #[test]
    fn it_does_not_create_with_invalid_opcode() {
        assert_eq!(OpCode::new(0), Err(OpCodeError::InvalidOpCode(0)));
        assert_eq!(OpCode::new(255), Err(OpCodeError::InvalidOpCode(255)));
    }

    #[test]
    fn it_reports_whether_opcode_takes_a_value() {
        assert!(OpCode::Set.takes_value());
        assert!(!OpCode::Get.takes_value());
        assert!(!OpCode::Delete.takes_value());
    }
}
