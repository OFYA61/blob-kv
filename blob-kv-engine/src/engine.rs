use std::{collections::HashMap, io, path::Path};

use crate::{
    log::{Log, LogCreateError, RecordScanError},
    record::{Record, RecordCreateError, RecordOp},
};

pub struct Engine {
    log: Log,
    index: HashMap<String, u64>,
}

impl Engine {
    pub fn new(log_path: impl AsRef<Path>) -> Result<Self, EngineCreateError> {
        let log = Log::new(log_path).map_err(|e| EngineCreateError::LogCreate(e))?;
        let mut index = HashMap::new();
        let records = log.scan_full().map_err(|e| EngineCreateError::LogScan(e))?;

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
                let record = self
                    .log
                    .scan_record(*pos)
                    .map_err(|e| EngineGetError::LogScan(e))?;
                Ok(Some(record.value.clone()))
            }
        }
    }

    pub fn set(&mut self, key: &str, value: &str) -> Result<(), EngineSetError> {
        let record =
            Record::new(key, value, RecordOp::Set).map_err(|e| EngineSetError::RecordCreate(e))?;
        let new_pos = self
            .log
            .append(&record)
            .map_err(|e| EngineSetError::LogAppend(e))?;
        self.index.insert(key.to_string(), new_pos);
        Ok(())
    }

    pub fn del(&mut self, key: &str) -> Result<bool, EngineDelError> {
        if !self.index.contains_key(key) {
            return Ok(false);
        }

        let del_record =
            Record::new(key, "", RecordOp::Del).map_err(|e| EngineDelError::RecordCreate(e))?;
        self.log
            .append(&del_record)
            .map_err(|e| EngineDelError::LogAppend(e))?;
        self.index.remove(key);

        Ok(true)
    }
}

#[derive(Debug)]
pub enum EngineCreateError {
    LogCreate(LogCreateError),
    LogScan(RecordScanError),
}

#[derive(Debug)]
pub enum EngineGetError {
    LogScan(RecordScanError),
}

#[derive(Debug)]
pub enum EngineSetError {
    RecordCreate(RecordCreateError),
    LogAppend(io::Error),
}

#[derive(Debug)]
pub enum EngineDelError {
    LogAppend(io::Error),
    RecordCreate(RecordCreateError),
}

#[cfg(test)]
mod test {}
