use std::{
    fs,
    io::{self, Seek, Write},
    os::unix::fs::FileExt,
    path::Path,
};

use crate::record::{Record, RecordDecodeError};

const HEADER_LENGTH: usize = 16; // last 6 bytes are reserved
const MAGIC_BYTES: [u8; 8] = [0x52, 0x55, 0x53, 0x54, 0x4B, 0x56, 0x44, 0x42];
const VERSION_BYTES_START: usize = MAGIC_BYTES.len();
const VERSION_BYTES_LENGTH: usize = 2;
const VERSION_EXPECTED: u16 = 1;

const fn default_header() -> [u8; HEADER_LENGTH] {
    let mut header = [0u8; HEADER_LENGTH];

    let mut i = 0;
    while i < MAGIC_BYTES.len() {
        header[i] = MAGIC_BYTES[i];
        i += 1;
    }

    let version = VERSION_EXPECTED.to_le_bytes();

    i = 0;
    while i < VERSION_BYTES_LENGTH {
        header[VERSION_BYTES_START + i] = version[i];
        i += 1;
    }

    header
}

const DEFAULT_HEADER: [u8; HEADER_LENGTH] = default_header();

macro_rules! log_io {
    ($expr: expr) => {
        $expr.map_err(|e| LogCreateError::Io(e))
    };
}

macro_rules! record_scan_io {
    ($expr:expr) => {
        $expr.map_err(|e| RecordScanError::Io(e))
    };
}

#[derive(Debug)]
pub struct Log {
    file: fs::File,
}

impl Log {
    pub fn new(path: impl AsRef<Path>) -> Result<Self, LogCreateError> {
        let exists = Path::new(path.as_ref()).exists();
        let mut file = log_io!(
            fs::OpenOptions::new()
                .create(true)
                .read(true)
                .append(true)
                .open(path)
        )?;
        file.try_lock().map_err(|e| LogCreateError::Lock(e))?;

        if exists {
            let mut header_buf = [0u8; HEADER_LENGTH];
            log_io!(file.read_exact_at(&mut header_buf, 0))?;

            if !header_buf[0..8].eq(&MAGIC_BYTES) {
                return Err(LogCreateError::InvalidMagic);
            }

            let version = u16::from_le_bytes(
                header_buf[VERSION_BYTES_START..VERSION_BYTES_START + VERSION_BYTES_LENGTH]
                    .try_into()
                    .unwrap(),
            );
            if version != VERSION_EXPECTED {
                return Err(LogCreateError::InvalidVersion);
            }
        } else {
            log_io!(file.write_all(&DEFAULT_HEADER))?;
        }

        Ok(Self { file })
    }

    pub fn append(&mut self, record: &Record) -> io::Result<u64> {
        let encoding = record.encode();
        let position = self.file.seek(io::SeekFrom::End(0))?;
        self.file.write_all(&encoding)?;
        Ok(position)
    }

    pub fn scan_full(&mut self) -> Result<Vec<RecordWrap>, RecordScanError> {
        let file_size = record_scan_io!(self.file.metadata())?.len();
        let mut offset = HEADER_LENGTH as u64;
        let mut records = Vec::new();

        while offset < file_size {
            let bytes = self.scan_record_bytes(offset)?;
            let record = match Record::decode(&bytes) {
                Ok(r) => r,
                Err(err) => {
                    println!(
                        "Failed to decode record at position={} with error={:?}",
                        offset, err
                    );
                    offset += bytes.len() as u64;
                    continue;
                }
            };

            records.push(RecordWrap {
                record,
                pos: offset,
            });
            offset += bytes.len() as u64;
        }

        Ok(records)
    }

    pub fn scan_record(&self, position: u64) -> Result<Record, RecordScanError> {
        let record = Record::decode(&self.scan_record_bytes(position)?)
            .map_err(|e| RecordScanError::Decoding(e))?;
        Ok(record)
    }

    fn scan_record_bytes(&self, position: u64) -> Result<Vec<u8>, RecordScanError> {
        let mut len_buf = [0u8; 4];
        record_scan_io!(self.file.read_exact_at(&mut len_buf, position))?;

        let len = u32::from_le_bytes(len_buf);
        let mut buffer = vec![0u8; len as usize];
        record_scan_io!(self.file.read_exact_at(&mut buffer, position))?;

        Ok(buffer)
    }
}

#[derive(Debug)]
pub enum LogCreateError {
    Io(io::Error),
    Lock(fs::TryLockError),
    InvalidMagic,
    InvalidVersion,
}

#[derive(Debug)]
pub enum RecordScanError {
    Io(io::Error),
    Decoding(RecordDecodeError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordWrap {
    pub record: Record,
    pub pos: u64,
}

#[cfg(test)]
mod test {
    use std::{
        assert_matches,
        fs::{File, OpenOptions},
        path::PathBuf,
    };

    use crate::record::RecordOp;

    use super::*;
    use tempfile::{TempDir, tempdir};

    const LOG: &str = "store.log";

    fn get_dir() -> TempDir {
        tempdir().expect("Failed to create temporary dir")
    }

    fn create_empty_log(dir: &TempDir) -> PathBuf {
        let path = dir.path().join(LOG);
        let mut file = File::create(&path).expect("Failed to create empty log");
        file.write_all(&DEFAULT_HEADER)
            .expect("Failed to write default header to log");
        path
    }

    #[test]
    fn it_creates_new_log_when_initializing() {
        let dir = get_dir();
        let path = dir.path().join(LOG);
        let log = Log::new(&path).expect("Failed to create log");
        assert!(path.exists());

        drop(log);
        let _ = Log::new(&path).expect("Failed to reopen log");
    }

    #[test]
    fn it_fails_to_initialize_when_log_file_is_locked() {
        let dir = get_dir();
        let path = dir.path().join(LOG);
        let log = Log::new(&path).expect("Failed to create log");
        assert!(path.exists());

        let result = Log::new(&path);
        assert_matches!(result, Err(LogCreateError::Lock(_)));

        drop(log);
    }

    #[test]
    fn it_opens_existing_log_when_initializing() {
        let dir = get_dir();
        let log_path = create_empty_log(&dir);
        let _ = Log::new(&log_path).expect("Failed to open existing log");
    }

    #[test]
    fn it_fails_opening_existing_log_with_invalid_magic() {
        let dir = get_dir();
        let path = dir.path().join(LOG);
        let mut file = File::create(&path).expect("Failed to create empty log");
        let mut header = DEFAULT_HEADER;
        header[0] = 0xFF;
        file.write_all(&header)
            .expect("Failed to write header to log");

        let result = Log::new(&path);
        assert_matches!(result, Err(LogCreateError::InvalidMagic));
    }

    #[test]
    fn it_fails_opening_existing_log_with_invalid_version() {
        let dir = get_dir();
        let path = dir.path().join(LOG);
        let mut file = File::create(&path).expect("Failed to create empty log");
        let mut header = DEFAULT_HEADER;
        header[VERSION_BYTES_START] = 0xFF;
        file.write_all(&header)
            .expect("Failed to write header to log");

        let result = Log::new(&path);
        assert_matches!(result, Err(LogCreateError::InvalidVersion));
    }

    #[test]
    fn it_appends() {
        let dir = get_dir();
        let path = dir.path().join(LOG);
        let mut log = Log::new(&path).expect("Failed to create log");

        let record = Record::new("hello", "world", RecordOp::Set).unwrap();

        let pos1 = log.append(&record).expect("Failed to append record");
        assert_eq!(pos1, HEADER_LENGTH as u64);
        let record_scan = log.scan_record(pos1).expect("Failed to scan record");
        assert_eq!(record, record_scan);
    }

    #[test]
    fn it_appends_and_retrieves_multiple_records() {
        let dir = get_dir();
        let path = dir.path().join(LOG);
        let mut log = Log::new(&path).expect("Failed to create log");

        let record1 = Record::new("hello", "world", RecordOp::Set).unwrap();
        let record2 = Record::new("hello2", "world2", RecordOp::Set).unwrap();
        let record3 = Record::new("hello3", "", RecordOp::Set).unwrap();

        let pos1 = log.append(&record1).expect("Failed to append record1");
        let pos2 = log.append(&record2).expect("Failed to append record2");
        let pos3 = log.append(&record3).expect("Failed to append record3");

        assert_eq!(pos1, HEADER_LENGTH as u64);
        assert_eq!(pos2, (HEADER_LENGTH + record1.encode_size()) as u64);
        assert_eq!(
            pos3,
            (HEADER_LENGTH + record1.encode_size() + record2.encode_size()) as u64
        );

        let record1_scan = log.scan_record(pos1).expect("Failed to scan record1");
        let record2_scan = log.scan_record(pos2).expect("Failed to scan record2");
        let record3_scan = log.scan_record(pos3).expect("Failed to scan record3");
        assert_eq!(record1, record1_scan);
        assert_eq!(record2, record2_scan);
        assert_eq!(record3, record3_scan);
    }

    #[test]
    fn it_scans_multiple_records() {
        let dir = get_dir();
        let path = dir.path().join(LOG);
        let mut log = Log::new(&path).expect("Failed to create log");

        let record1 = Record::new("hello", "world", RecordOp::Set).unwrap();
        let record2 = Record::new("hello2", "", RecordOp::Set).unwrap();
        let record3 = Record::new("hello3", "world3", RecordOp::Set).unwrap();

        let pos1 = log.append(&record1).expect("Failed to append record1");
        let pos2 = log.append(&record2).expect("Failed to append record2");
        let pos3 = log.append(&record3).expect("Failed to append record3");

        let scan = log.scan_full().expect("Failed to scan records");
        assert_eq!(scan.len(), 3);
        assert_eq!(scan[0].record, record1);
        assert_eq!(scan[0].pos, pos1);
        assert_eq!(scan[1].record, record2);
        assert_eq!(scan[1].pos, pos2);
        assert_eq!(scan[2].record, record3);
        assert_eq!(scan[2].pos, pos3);
    }

    #[test]
    fn it_rejects_invalid_record_positions() {
        let dir = get_dir();
        let path = dir.path().join(LOG);
        let mut log = Log::new(&path).expect("Failed to create log");

        let record = Record::new("hello", "world", RecordOp::Set).unwrap();
        let pos = log.append(&record).expect("Failed to append record");

        let result = log.scan_record(pos + 1);
        assert_matches!(result, Err(_));
    }

    #[test]
    fn it_ignores_invalid_logs_on_scan() {
        let dir = get_dir();
        let path = dir.path().join(LOG);
        let mut log = Log::new(&path).expect("Failed to create log");

        let record1 = Record::new("hello", "world", RecordOp::Set).unwrap();
        let record2 = Record::new("hello2", "", RecordOp::Set).unwrap();
        let record3 = Record::new("hello3", "world3", RecordOp::Set).unwrap();

        let _ = log.append(&record1).expect("Failed to append record1");
        let pos2 = log.append(&record2).expect("Failed to append record2");
        let pos3 = log.append(&record3).expect("Failed to append record3");

        // Close the log and modify the checksum of record1
        drop(log);
        let file = OpenOptions::new()
            .write(true)
            .open(&path)
            .expect("Failed to open log file");
        file.write_at(&[0xFF], pos2 - 1)
            .expect("Faild to overrwite checksum for record1");
        drop(file);

        let mut log = Log::new(&path).expect("Failed to reopne log");

        let scan = log.scan_full().expect("Failed to scan records");
        assert_eq!(scan.len(), 2);
        assert_eq!(scan[0].record, record2);
        assert_eq!(scan[0].pos, pos2);
        assert_eq!(scan[1].record, record3);
        assert_eq!(scan[1].pos, pos3);
    }
}
