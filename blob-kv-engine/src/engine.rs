use std::{collections::HashMap, io, path::Path};

use crate::{
    log::{Log, LogInitError, RecordScanError},
    record::{Record, RecordCreateError, RecordOp},
};

pub struct Engine {
    log: Log,
    index: HashMap<String, u64>,
}

impl Engine {
    pub fn init(log_path: impl AsRef<Path>) -> Result<Self, EngineInitError> {
        let log = Log::init(log_path)?;
        let mut index = HashMap::new();
        let records = log.scan_full()?;

        for record_wrap in records {
            let key = record_wrap.record.key;
            let pos = record_wrap.pos;
            match record_wrap.record.op {
                RecordOp::Set => index.insert(key, pos),
                RecordOp::Del => index.remove(&key),
            };
        }

        Ok(Self { log, index })
    }

    pub fn get(&self, key: &str) -> Result<Option<String>, EngineGetError> {
        match self.index.get(key) {
            None => Ok(None),
            Some(pos) => {
                let record = self.log.scan_record(*pos)?;
                Ok(Some(record.value.clone()))
            }
        }
    }

    pub fn set(&mut self, key: &str, value: &str) -> Result<(), EngineSetError> {
        let record = Record::new(key, value, RecordOp::Set)?;
        let new_pos = self.log.append(&record)?;
        self.index.insert(key.to_string(), new_pos);
        Ok(())
    }

    pub fn del(&mut self, key: &str) -> Result<bool, EngineDelError> {
        if !self.index.contains_key(key) {
            return Ok(false);
        }

        let del_record = Record::new(key, "", RecordOp::Del)?;
        self.log.append(&del_record)?;
        self.index.remove(key);

        Ok(true)
    }
}

#[derive(thiserror::Error, Debug)]
pub enum EngineInitError {
    #[error("failed to initialize log: {0:?}")]
    LogInit(#[from] LogInitError),
    #[error("failed to scan log: {0:?}")]
    LogScan(#[from] RecordScanError),
}

#[derive(thiserror::Error, Debug)]
pub enum EngineGetError {
    #[error("failed to scan log: {0:?}")]
    LogScan(#[from] RecordScanError),
}

#[derive(thiserror::Error, Debug)]
pub enum EngineSetError {
    #[error("failed to append record to log: {0:?}")]
    LogAppend(#[from] io::Error),
    #[error("failed to create record: {0:?}")]
    RecordCreate(#[from] RecordCreateError),
}

#[derive(thiserror::Error, Debug)]
pub enum EngineDelError {
    #[error("failed to append record to log: {0:?}")]
    LogAppend(#[from] io::Error),
    #[error("failed to create record: {0:?}")]
    RecordCreate(#[from] RecordCreateError),
}

#[cfg(test)]
mod test {
    use std::{
        assert_matches,
        fs::{self, OpenOptions},
        os::unix::fs::FileExt,
        path::{Path, PathBuf},
    };

    use crate::record::RecordDecodeError;
    use tempfile::{TempDir, tempdir};

    use super::*;

    const LOG: &str = "store.log";

    fn get_dir() -> TempDir {
        tempdir().expect("Failed to create temporary dir")
    }

    fn get_log_path(dir: &TempDir) -> PathBuf {
        dir.path().join(LOG)
    }

    fn create_engine(dir: &TempDir) -> Engine {
        Engine::init(get_log_path(dir)).expect("Failed to create engine")
    }

    fn flip_byte(path: &Path, position: u64) {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .expect("Failed to open log file");

        let mut byte = [0u8; 1];
        file.read_exact_at(&mut byte, position)
            .expect("Failed to read log byte");
        byte[0] ^= 0xFF;
        file.write_at(&byte, position)
            .expect("Failed to corrupt log byte");
    }

    #[test]
    fn it_gets_existing_key() {
        let dir = get_dir();
        let mut engine = create_engine(&dir);

        engine.set("hello", "world").expect("Failed to set key");

        let value = engine.get("hello").expect("Failed to get key");
        assert_eq!(value, Some("world".to_string()));
    }

    #[test]
    fn it_returns_none_when_key_does_not_exist() {
        let dir = get_dir();
        let engine = create_engine(&dir);

        let value = engine.get("missing").expect("Failed to get key");
        assert_eq!(value, None);
    }

    #[test]
    fn it_gets_overwritten_value() {
        let dir = get_dir();
        let mut engine = create_engine(&dir);

        engine.set("hello", "world").expect("Failed to set key");
        engine
            .set("hello", "mars")
            .expect("Failed to overwrite key");

        let value = engine.get("hello").expect("Failed to get key");
        assert_eq!(value, Some("mars".to_string()));
    }

    #[test]
    fn it_gets_empty_value() {
        let dir = get_dir();
        let mut engine = create_engine(&dir);

        engine.set("empty", "").expect("Failed to set empty value");

        let value = engine.get("empty").expect("Failed to get key");
        assert_eq!(value, Some(String::new()));
    }

    #[test]
    fn it_gets_unicode_key_and_value() {
        let dir = get_dir();
        let mut engine = create_engine(&dir);

        let key = "clé-日本語-🔑";
        let value = "héllo 🌍 — Ω";
        engine.set(key, value).expect("Failed to set key");

        let value = engine.get(key).expect("Failed to get key");
        assert_eq!(value, Some("héllo 🌍 — Ω".to_string()));
    }

    #[test]
    fn it_gets_multiple_keys() {
        let dir = get_dir();
        let mut engine = create_engine(&dir);

        engine.set("one", "1").expect("Failed to set key one");
        engine.set("two", "2").expect("Failed to set key two");

        let one = engine.get("one").expect("Failed to get key one");
        let two = engine.get("two").expect("Failed to get key two");
        assert_eq!(one, Some("1".to_string()));
        assert_eq!(two, Some("2".to_string()));
    }

    #[test]
    fn it_fails_to_get_when_record_is_corrupt() {
        let dir = get_dir();
        let path = get_log_path(&dir);
        let mut engine = create_engine(&dir);

        engine.set("hello", "world").expect("Failed to set key");
        let pos = *engine.index.get("hello").expect("Failed to find key");
        let record_len = Record::new("hello", "world", RecordOp::Set)
            .expect("Failed to create record")
            .encode()
            .len() as u64;

        flip_byte(&path, pos + record_len - 1);

        let result = engine.get("hello");
        assert_matches!(
            result,
            Err(EngineGetError::LogScan(RecordScanError::Decoding(
                RecordDecodeError::ChecksumMismatch
            )))
        );
    }

    #[test]
    fn it_deletes_key() {
        let dir = get_dir();
        let mut engine = create_engine(&dir);

        engine.set("hello", "world").expect("Failed to set key");
        let deleted = engine.del("hello").expect("Failed to delete key");
        assert!(deleted);

        let value = engine.get("hello").expect("Failed to get key");
        assert_eq!(value, None);
    }

    #[test]
    fn it_returns_false_when_deleting_missing_key() {
        let dir = get_dir();
        let mut engine = create_engine(&dir);

        let deleted = engine.del("missing").expect("Failed to delete key");
        assert!(!deleted);

        let value = engine.get("missing").expect("Failed to get key");
        assert_eq!(value, None);
    }

    #[test]
    fn it_does_not_append_when_deleting_missing_key() {
        let dir = get_dir();
        let path = get_log_path(&dir);
        let mut engine = create_engine(&dir);

        engine.set("hello", "world").expect("Failed to set key");
        let len_before = fs::metadata(&path)
            .expect("Failed to read log metadata")
            .len();

        let deleted = engine.del("missing").expect("Failed to delete key");
        assert!(!deleted);

        let len_after = fs::metadata(&path)
            .expect("Failed to read log metadata")
            .len();
        assert_eq!(len_before, len_after);
    }

    #[test]
    fn it_persists_values_across_reopen() {
        let dir = get_dir();
        let path = get_log_path(&dir);
        let mut engine = create_engine(&dir);

        engine.set("hello", "world").expect("Failed to set key");
        engine.set("goodbye", "moon").expect("Failed to set key");
        drop(engine);

        let engine = Engine::init(&path).expect("Failed to reopen engine");
        let hello = engine.get("hello").expect("Failed to get key");
        let goodbye = engine.get("goodbye").expect("Failed to get key");
        assert_eq!(hello, Some("world".to_string()));
        assert_eq!(goodbye, Some("moon".to_string()));
    }

    #[test]
    fn it_persists_overwritten_value_across_reopen() {
        let dir = get_dir();
        let path = get_log_path(&dir);
        let mut engine = create_engine(&dir);

        engine.set("hello", "world").expect("Failed to set key");
        engine
            .set("hello", "mars")
            .expect("Failed to overwrite key");
        drop(engine);

        let engine = Engine::init(&path).expect("Failed to reopen engine");
        let value = engine.get("hello").expect("Failed to get key");
        assert_eq!(value, Some("mars".to_string()));
    }

    #[test]
    fn it_persists_deletion_across_reopen() {
        let dir = get_dir();
        let path = get_log_path(&dir);
        let mut engine = create_engine(&dir);

        engine.set("hello", "world").expect("Failed to set key");
        let deleted = engine.del("hello").expect("Failed to delete key");
        assert!(deleted);
        drop(engine);

        let engine = Engine::init(&path).expect("Failed to reopen engine");
        let value = engine.get("hello").expect("Failed to get key");
        assert_eq!(value, None);
    }

    #[test]
    fn it_gets_new_value_after_deleting_and_setting_again() {
        let dir = get_dir();
        let mut engine = create_engine(&dir);

        engine.set("hello", "world").expect("Failed to set key");
        let deleted = engine.del("hello").expect("Failed to delete key");
        assert!(deleted);
        engine.set("hello", "again").expect("Failed to set key");

        let value = engine.get("hello").expect("Failed to get key");
        assert_eq!(value, Some("again".to_string()));
    }

    #[test]
    fn it_fails_to_rebuild_index_when_a_record_is_corrupt() {
        let dir = get_dir();
        let path = get_log_path(&dir);
        let mut engine = create_engine(&dir);

        engine.set("first", "one").expect("Failed to set key");
        engine.set("second", "two").expect("Failed to set key");

        let first_pos = *engine.index.get("first").expect("Failed to find key");
        let first_len = Record::new("first", "one", RecordOp::Set)
            .expect("Failed to create record")
            .encode()
            .len() as u64;
        drop(engine);

        flip_byte(&path, first_pos + first_len - 1);

        let result = Engine::init(&path);
        assert!(matches!(
            result,
            Err(EngineInitError::LogScan(RecordScanError::DecodingFailures(failures)))
                if failures.len() == 1
                    && failures[0].position == first_pos
                    && failures[0].error == RecordDecodeError::ChecksumMismatch
        ));
    }

    #[test]
    fn it_rejects_empty_key_on_set() {
        let dir = get_dir();
        let mut engine = create_engine(&dir);

        let result = engine.set("", "world");
        assert_matches!(
            result,
            Err(EngineSetError::RecordCreate(RecordCreateError::InvalidKey(
                _
            )))
        );

        let value = engine.get("").expect("Failed to get key");
        assert_eq!(value, None);
    }

    #[test]
    fn it_fails_to_create_engine_when_log_is_locked() {
        let dir = get_dir();
        let path = get_log_path(&dir);
        let engine = Engine::init(&path).expect("Failed to create engine");

        let result = Engine::init(&path);
        assert!(matches!(
            result,
            Err(EngineInitError::LogInit(LogInitError::Lock(_)))
        ));

        drop(engine);
    }
}
