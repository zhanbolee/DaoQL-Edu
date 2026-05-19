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
//! Column file / chunk file management
//!
//! Educational Notes:
//! - column store uses independent file：one per column .col file, each chunk a .dat file
//! - file format：Header + Data（optional lz4 compress）
//! - Header contains magic, version, metadata, used for startup validation

use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, Write};
use std::path::{Path, PathBuf};

use crate::error::{DaoQLError, StorageError};

/// columnfile header
///
/// 256 bytesfixedhead，containfilemetadata。
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ColumnFileHeader {
    /// magic number
    pub magic: [u8; 4],
    /// Version number
    pub version: u32,
    /// Field type
    pub field_type: u8,
    /// Granule quantity
    pub granule_count: u32,
    /// Total record count
    pub total_records: u64,
    /// Reserved
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

    /// validate magic number
    pub fn validate(&self) -> Result<(), DaoQLError> {
        if self.magic != Self::MAGIC {
            return Err(DaoQLError::Storage(StorageError::Corruption(
                "columnfilemagic numbermismatch".to_string(),
            )));
        }
        Ok(())
    }

    /// Serialize to byte countgroup
    pub fn to_bytes(&self) -> [u8; 256] {
        let mut buf = [0u8; 256];
        buf[0..4].copy_from_slice(&self.magic);
        buf[4..8].copy_from_slice(&self.version.to_le_bytes());
        buf[8] = self.field_type;
        buf[9..13].copy_from_slice(&self.granule_count.to_le_bytes());
        buf[13..21].copy_from_slice(&self.total_records.to_le_bytes());
        buf
    }

    /// Secondarybyte countgroupParse
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

/// columnfilemanager
pub struct ColumnFile {
    pub path: PathBuf,
    pub file: File,
    pub header: ColumnFileHeader,
}

impl ColumnFile {
    /// OpenOrCreatecolumnfile
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

    /// append granule data
    pub fn append_granule(&mut self, data: &[u8]) -> Result<u64, DaoQLError> {
        let offset = self.file.metadata()?.len();
        self.file.write_all(data)?;
        self.header.granule_count += 1;
        self.header.total_records += 1;
        self.sync_header()?;
        Ok(offset)
    }

    /// Read granule data
    pub fn read_at(&mut self, offset: u64, len: usize) -> Result<Vec<u8>, DaoQLError> {
        self.file.seek(std::io::SeekFrom::Start(offset))?;
        let mut buf = vec![0u8; len];
        self.file.read_exact(&mut buf)?;
        Ok(buf)
    }

    /// Synchronousfile header
    fn sync_header(&mut self) -> Result<(), DaoQLError> {
        self.file.seek(std::io::SeekFrom::Start(0))?;
        self.file.write_all(&self.header.to_bytes())?;
        self.file.sync_data()?;
        Ok(())
    }
}

/// Chunk file（RawLayer）
///
/// Storage raw row data file。
pub struct ChunkFile {
    pub path: PathBuf,
    pub file: File,
    pub chunk_id: u64,
    pub record_count: u32,
}

impl ChunkFile {
    pub const MAGIC: [u8; 4] = *b"RAWH";

    /// Createnew chunk file
    pub fn create(path: impl AsRef<Path>, chunk_id: u64) -> Result<Self, DaoQLError> {
        let path = path.as_ref().to_path_buf();
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)?;

        // Writehead
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

    /// Append record
    pub fn append_record(&mut self, data: &[u8]) -> Result<u64, DaoQLError> {
        let offset = self.file.metadata()?.len();
        let len = data.len() as u32;
        self.file.write_all(&len.to_le_bytes())?;
        self.file.write_all(data)?;
        self.record_count += 1;
        Ok(offset)
    }

    /// Readallrecord
    pub fn read_all(&mut self) -> Result<Vec<Vec<u8>>, DaoQLError> {
        self.file.seek(std::io::SeekFrom::Start(256))?; // skiphead
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

        // re-openvalidate
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
