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
//! 列文件 / chunk 文件管理
//!
//! 教学说明：
//! - 列存储使用独立文件：每列一个 .col 文件，每个 chunk 一个 .dat 文件
//! - 文件格式：Header + Data（可选 lz4 压缩）
//! - Header 包含 magic、版本、元数据，用于启动时校验

use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, Write};
use std::path::{Path, PathBuf};

use crate::error::{DaoQLError, StorageError};

/// 列文件头
///
/// 256 字节固定头，包含文件元数据。
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ColumnFileHeader {
    /// 魔数
    pub magic: [u8; 4],
    /// 版本号
    pub version: u32,
    /// 字段类型
    pub field_type: u8,
    /// Granule 数量
    pub granule_count: u32,
    /// 总记录数
    pub total_records: u64,
    /// 预留
    pub reserved: [u8; 240],
}

impl ColumnFileHeader {
    pub const MAGIC: [u8; 4] = *b"COLH";
    pub const SIZE: usize = 256;

    pub fn new(field_type: u8) -> Self {
        Self {
            magic: Self::MAGIC,
            version: 1,
            field_type,
            granule_count: 0,
            total_records: 0,
            reserved: [0; 240],
        }
    }

    /// 校验魔数
    pub fn validate(&self) -> Result<(), DaoQLError> {
        if self.magic != Self::MAGIC {
            return Err(DaoQLError::Storage(StorageError::Corruption(
                "列文件魔数不匹配".to_string(),
            )));
        }
        Ok(())
    }

    /// 序列化为字节数组
    pub fn to_bytes(&self) -> [u8; 256] {
        let mut buf = [0u8; 256];
        buf[0..4].copy_from_slice(&self.magic);
        buf[4..8].copy_from_slice(&self.version.to_le_bytes());
        buf[8] = self.field_type;
        buf[9..13].copy_from_slice(&self.granule_count.to_le_bytes());
        buf[13..21].copy_from_slice(&self.total_records.to_le_bytes());
        buf
    }

    /// 从字节数组解析
    pub fn from_bytes(bytes: &[u8; 256]) -> Self {
        Self {
            magic: [bytes[0], bytes[1], bytes[2], bytes[3]],
            version: u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]),
            field_type: bytes[8],
            granule_count: u32::from_le_bytes([bytes[9], bytes[10], bytes[11], bytes[12]]),
            total_records: u64::from_le_bytes([
                bytes[13], bytes[14], bytes[15], bytes[16],
                bytes[17], bytes[18], bytes[19], bytes[20],
            ]),
            reserved: [0; 240],
        }
    }
}

/// 列文件管理器
pub struct ColumnFile {
    pub path: PathBuf,
    pub file: File,
    pub header: ColumnFileHeader,
}

impl ColumnFile {
    /// 打开或创建列文件
    pub fn open_or_create(path: impl AsRef<Path>, field_type: u8) -> Result<Self, DaoQLError> {
        let path = path.as_ref().to_path_buf();
        let exists = path.exists();

        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)?;

        let header = if exists {
            let mut buf = [0u8; 256];
            file.read_exact(&mut buf)?;
            let h = ColumnFileHeader::from_bytes(&buf);
            h.validate()?;
            h
        } else {
            let h = ColumnFileHeader::new(field_type);
            file.write_all(&h.to_bytes())?;
            file.sync_all()?;
            h
        };

        Ok(Self { path, file, header })
    }

    /// 追加 granule 数据
    pub fn append_granule(&mut self, data: &[u8]) -> Result<u64, DaoQLError> {
        let offset = self.file.metadata()?.len();
        self.file.write_all(data)?;
        self.header.granule_count += 1;
        self.header.total_records += 1;
        self.sync_header()?;
        Ok(offset)
    }

    /// 读取 granule 数据
    pub fn read_at(&mut self, offset: u64, len: usize) -> Result<Vec<u8>, DaoQLError> {
        self.file.seek(std::io::SeekFrom::Start(offset))?;
        let mut buf = vec![0u8; len];
        self.file.read_exact(&mut buf)?;
        Ok(buf)
    }

    /// 同步文件头
    fn sync_header(&mut self) -> Result<(), DaoQLError> {
        self.file.seek(std::io::SeekFrom::Start(0))?;
        self.file.write_all(&self.header.to_bytes())?;
        self.file.sync_data()?;
        Ok(())
    }
}

/// Chunk 文件（RawLayer）
///
/// 存储原始行数据的文件。
pub struct ChunkFile {
    pub path: PathBuf,
    pub file: File,
    pub chunk_id: u64,
    pub record_count: u32,
}

impl ChunkFile {
    pub const MAGIC: [u8; 4] = *b"RAWH";

    /// 创建新的 chunk 文件
    pub fn create(path: impl AsRef<Path>, chunk_id: u64) -> Result<Self, DaoQLError> {
        let path = path.as_ref().to_path_buf();
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)?;

        // 写入头
        let mut header = vec![0u8; 256];
        header[0..4].copy_from_slice(&Self::MAGIC);
        header[4..8].copy_from_slice(&1u32.to_le_bytes()); // version
        header[8..16].copy_from_slice(&chunk_id.to_le_bytes());
        file.write_all(&header)?;
        file.sync_data()?;

        Ok(Self {
            path,
            file,
            chunk_id,
            record_count: 0,
        })
    }

    /// 追加记录
    pub fn append_record(&mut self, data: &[u8]) -> Result<u64, DaoQLError> {
        let offset = self.file.metadata()?.len();
        let len = data.len() as u32;
        self.file.write_all(&len.to_le_bytes())?;
        self.file.write_all(data)?;
        self.record_count += 1;
        Ok(offset)
    }

    /// 读取所有记录
    pub fn read_all(&mut self) -> Result<Vec<Vec<u8>>, DaoQLError> {
        self.file.seek(std::io::SeekFrom::Start(256))?; // 跳过头
        let mut records = Vec::new();
        let file_len = self.file.metadata()?.len();

        while self.file.stream_position()? < file_len {
            let mut len_buf = [0u8; 4];
            if self.file.read_exact(&mut len_buf).is_err() {
                break;
            }
            let len = u32::from_le_bytes(len_buf) as usize;
            let mut buf = vec![0u8; len];
            self.file.read_exact(&mut buf)?;
            records.push(buf);
        }

        Ok(records)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn temp_path(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join("daoql-edu-test-file");
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(name)
    }

    #[test]
    fn test_column_file_header_roundtrip() {
        let h = ColumnFileHeader::new(2);
        let bytes = h.to_bytes();
        let h2 = ColumnFileHeader::from_bytes(&bytes);
        assert_eq!(h.magic, h2.magic);
        assert_eq!(h.version, h2.version);
        assert_eq!(h.field_type, h2.field_type);
    }

    #[test]
    fn test_column_file_create_and_validate() {
        let path = temp_path("test_col.dat");
        let _ = std::fs::remove_file(&path);

        let mut cf = ColumnFile::open_or_create(&path, 1).unwrap();
        assert_eq!(cf.header.field_type, 1);

        let data = b"test granule data";
        let offset = cf.append_granule(data).unwrap();
        assert!(offset > 0);

        // 重新打开验证
        drop(cf);
        let cf2 = ColumnFile::open_or_create(&path, 1).unwrap();
        assert_eq!(cf2.header.granule_count, 1);
    }

    #[test]
    fn test_chunk_file_append_and_read() {
        let path = temp_path("test_chunk.dat");
        let _ = std::fs::remove_file(&path);

        let mut cf = ChunkFile::create(&path, 0).unwrap();

        cf.append_record(b"record1").unwrap();
        cf.append_record(b"record2").unwrap();

        let records = cf.read_all().unwrap();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0], b"record1");
        assert_eq!(records[1], b"record2");
    }
}
