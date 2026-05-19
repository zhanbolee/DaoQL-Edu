// Copyright (c) 2026 Zhanbo Li / Atlas Lee <4859345@qq.com>
// SPDX-License-Identifier: BSL-1.1
//
// Licensed under the Business Source License, version 1.1 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at:
//
//     https://spdx.org/licenses/BSL-1.1.html
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
//
//! WAL Crash Recovery
//!
//! Educational Notes:
//! - at startup scan WAL file, parse all valid records
//! - sort by seq, replay already committed transactions
//! - idempotent replay: if data already exists, skip (via BeingId deduplication)
//! - data after corrupted record is considered invalid (WAL is sequential log)

use std::fs::File;
use std::io::Read;
use std::path::Path;

use crate::error::DaoQLError;
use crate::wal::record::{TransactionPayload, WalRecord, WAL_MAGIC};

/// WAL recovery
pub struct WalRecovery;

impl WalRecovery {
    /// Scan WAL file，returnall valid records
    pub fn scan(path: impl AsRef<Path>) -> Result<Vec<WalRecord>, DaoQLError> {
        let path = path.as_ref();
        if !path.exists() {
            return Ok(Vec::new());
        }

        let mut file = File::open(path)?;
        let mut data = Vec::new();
        file.read_to_end(&mut data)?;

        if data.len() < 8 {
            return Ok(Vec::new());
        }

        // checkfile headermagic number
        if data[0..4] != WAL_MAGIC {
            return Err(DaoQLError::Wal("WAL file magic number mismatch".to_string()));
        }

        // skipfile header（8 bytes）
        let mut offset = 8;
        let mut records = Vec::new();

        while offset < data.len() {
            match WalRecord::parse_one(&data[offset..]) {
                Ok((Some(record), consumed)) => {
                    offset += consumed;
                    records.push(record);
                }
                Ok((None, _)) => {
                    // data incomplete，stop
                    break;
                }
                Err(e) => {
                    // record corrupted, stop (subsequent records considered invalid)
                    eprintln!("WAL recovery: record corrupted at offset {offset}: {e}");
                    break;
                }
            }
        }

        // sort by seq
        records.sort_by_key(|r| r.seq);
        Ok(records)
    }

    /// replay WAL record
    ///
    /// edu edition simplification：only parse and return transaction payload, actual redo executed by upper layer caller。
    /// Production version should directly modify storage here。
    pub fn replay(path: impl AsRef<Path>) -> Result<Vec<TransactionPayload>, DaoQLError> {
        let records = Self::scan(path)?;
        let mut payloads = Vec::with_capacity(records.len());

        for record in records {
            match TransactionPayload::from_bytes(&record.payload) {
                Ok(payload) => payloads.push(payload),
                Err(e) => {
                    eprintln!("WAL recovery: deserializefailed seq={}: {e}", record.seq);
                    // skip corrupted payload, continue
                }
            }
        }

        Ok(payloads)
    }

    /// Get last valid sequence number
    pub fn last_seq(path: impl AsRef<Path>) -> Result<u64, DaoQLError> {
        let records = Self::scan(path)?;
        Ok(records.last().map(|r| r.seq).unwrap_or(0))
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;
    use crate::wal::record::TransactionPayload;
    use crate::wal::writer::WalWriter;

    fn temp_path(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join("daoql-edu-test-recovery");
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(name)
    }

    #[test]
    fn test_recovery_empty() {
        let path = temp_path("empty.wal");
        let _ = std::fs::remove_file(&path);
        let records = WalRecovery::scan(&path).unwrap();
        assert!(records.is_empty());
    }

    #[test]
    fn test_recovery_scan() {
        let path = temp_path("scan.wal");
        let _ = std::fs::remove_file(&path);

        {
            let mut writer = WalWriter::new(&path, 4096, 1000, true).unwrap();
            for i in 0..5 {
                let payload = TransactionPayload::new(i);
                let record = WalRecord::new(i + 1, payload.to_bytes().unwrap());
                writer.append(&record).unwrap();
            }
            writer.shutdown().unwrap();
        }

        let records = WalRecovery::scan(&path).unwrap();
        assert_eq!(records.len(), 5);
        assert_eq!(records[0].seq, 1);
        assert_eq!(records[4].seq, 5);
    }

    #[test]
    fn test_recovery_replay() {
        let path = temp_path("replay.wal");
        let _ = std::fs::remove_file(&path);

        {
            let mut writer = WalWriter::new(&path, 4096, 1000, true).unwrap();
            for i in 0..3 {
                let payload = TransactionPayload::new(i);
                let record = WalRecord::new(i + 1, payload.to_bytes().unwrap());
                writer.append(&record).unwrap();
            }
            writer.shutdown().unwrap();
        }

        let payloads = WalRecovery::replay(&path).unwrap();
        assert_eq!(payloads.len(), 3);
        assert_eq!(payloads[0].tx_id, 0);
        assert_eq!(payloads[1].tx_id, 1);
        assert_eq!(payloads[2].tx_id, 2);
    }

    #[test]
    fn test_recovery_corruption() {
        let path = temp_path("corrupt.wal");
        let _ = std::fs::remove_file(&path);

        // Write valid data + garbage
        {
            let mut writer = WalWriter::new(&path, 4096, 1000, true).unwrap();
            let payload = TransactionPayload::new(1);
            let record = WalRecord::new(1, payload.to_bytes().unwrap());
            writer.append(&record).unwrap();
            writer.shutdown().unwrap();
        }

        // append garbage
        {
            let mut file = std::fs::OpenOptions::new()
                .append(true)
                .open(&path)
                .unwrap();
            file.write_all(b"GARBAGE").unwrap();
        }

        let records = WalRecovery::scan(&path).unwrap();
        assert_eq!(records.len(), 1); // only recover valid records
    }
}
