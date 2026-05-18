// Copyright (c) 2026 黎展波 / Atlas Lee <4859345@qq.com>
// SPDX-License-Identifier: BSL-1.1
//
// Licensed under the Business Source License, version 1.1 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at:
//
//     https://mariadb.com/bsl11/
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
//
//! WAL 崩溃恢复
//!
//! 教学说明：
//! - 启动时扫描 WAL 文件，解析所有有效记录
//! - 按 seq 排序，回放已提交事务
//! - 幂等回放：如果数据已存在，跳过（通过 BeingId 去重）
//! - 损坏记录后的数据被视为无效（WAL 是顺序日志）

use std::fs::File;
use std::io::Read;
use std::path::Path;

use crate::error::DaoQLError;
use crate::wal::record::{TransactionPayload, WalRecord, WAL_MAGIC};

/// WAL 恢复器
pub struct WalRecovery;

impl WalRecovery {
    /// 扫描 WAL 文件，返回所有有效记录
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

        // 检查文件头魔数
        if data[0..4] != WAL_MAGIC {
            return Err(DaoQLError::Wal("WAL 文件魔数不匹配".to_string()));
        }

        // 跳过文件头（8 bytes）
        let mut offset = 8;
        let mut records = Vec::new();

        while offset < data.len() {
            match WalRecord::parse_one(&data[offset..]) {
                Ok((Some(record), consumed)) => {
                    offset += consumed;
                    records.push(record);
                }
                Ok((None, _)) => {
                    // 数据不完整，停止
                    break;
                }
                Err(e) => {
                    // 记录损坏，停止（后续记录视为无效）
                    eprintln!("WAL 恢复: 记录损坏于偏移 {offset}: {e}");
                    break;
                }
            }
        }

        // 按 seq 排序
        records.sort_by_key(|r| r.seq);
        Ok(records)
    }

    /// 回放 WAL 记录
    ///
    /// 教学版简化：仅解析并返回事务负载，实际重做由上层调用者执行。
    /// 生产版应在此直接修改存储。
    pub fn replay(path: impl AsRef<Path>) -> Result<Vec<TransactionPayload>, DaoQLError> {
        let records = Self::scan(path)?;
        let mut payloads = Vec::with_capacity(records.len());

        for record in records {
            match TransactionPayload::from_bytes(&record.payload) {
                Ok(payload) => payloads.push(payload),
                Err(e) => {
                    eprintln!("WAL 恢复: 反序列化失败 seq={}: {e}", record.seq);
                    // 跳过损坏的 payload，继续
                }
            }
        }

        Ok(payloads)
    }

    /// 获取最后有效序列号
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

        // 写入有效数据 + 垃圾
        {
            let mut writer = WalWriter::new(&path, 4096, 1000, true).unwrap();
            let payload = TransactionPayload::new(1);
            let record = WalRecord::new(1, payload.to_bytes().unwrap());
            writer.append(&record).unwrap();
            writer.shutdown().unwrap();
        }

        // 追加垃圾
        {
            let mut file = std::fs::OpenOptions::new()
                .append(true)
                .open(&path)
                .unwrap();
            file.write_all(b"GARBAGE").unwrap();
        }

        let records = WalRecovery::scan(&path).unwrap();
        assert_eq!(records.len(), 1); // 只恢复有效记录
    }
}
