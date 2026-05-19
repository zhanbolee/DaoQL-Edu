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
//! Global Identifier System
//!
//! Educational Notes:
//! - BeingId = shard_id(2B) + UUID v7(16B) = 18B
//! - UUID v7 is time-sortable, making chronologically ordered inserts naturally sorted
//! - shard_id reserved for sharding (edu edition fixed at 0)
//! - Fixed-length design enables inline mmap storage

use std::fmt;
use std::hash::{Hash, Hasher};
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Globally unique entity identifier
///
/// Memory layout (18 bytes):
/// ```ignore
/// [shard_id: u16 | 2 bytes][uuid: Uuid | 16 bytes]
/// ```ignore
///
/// Benefits of UUID v7 (time-sortable UUID):
/// - Timestamp in high bits, sort by creation time without extra index
/// - Compatible with standard UUID toolchain
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct BeingId {
    pub shard_id: u16,
    pub uuid: Uuid,
}

impl BeingId {
    /// Fixed size 18 bytes
    pub const SIZE: usize = 18;

    /// Create new BeingId (shard_id fixed at 0, edu edition)
    pub fn new() -> Self {
        Self {
            shard_id: 0,
            uuid: Uuid::now_v7(),
        }
    }

    /// Create from shard_id + UUID
    pub fn from_parts(shard_id: u16, uuid: Uuid) -> Self {
        Self { shard_id, uuid }
    }

    /// Serialize to 18-byte array
    pub fn to_bytes(&self) -> [u8; 18] {
        let mut buf = [0u8; 18];
        buf[0..2].copy_from_slice(&self.shard_id.to_be_bytes());
        buf[2..18].copy_from_slice(self.uuid.as_bytes());
        buf
    }

    /// Parse from 18-byte array
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != 18 {
            return None;
        }
        let shard_id = u16::from_be_bytes([bytes[0], bytes[1]]);
        let uuid = Uuid::from_slice(&bytes[2..18]).ok()?;
        Some(Self { shard_id, uuid })
    }

    /// Get creation time (extracted from UUID v7)
    ///
    /// UUID v7 format:
    /// ```ignore
    /// 0                   1                   2                   3
    /// 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
    /// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
    /// |                           unix_ts_ms                          |
    /// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
    /// |          unix_ts_ms           |  ver  |       rand_a          |
    /// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
    /// |var|                        rand_b                             |
    /// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
    /// |                            rand_b                             |
    /// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
    /// ```
    pub fn created_at_ms(&self) -> u64 {
        let bytes = self.uuid.as_bytes();
        
        ((bytes[0] as u64) << 40)
            | ((bytes[1] as u64) << 32)
            | ((bytes[2] as u64) << 24)
            | ((bytes[3] as u64) << 16)
            | ((bytes[4] as u64) << 8)
            | (bytes[5] as u64)
    }

    /// Convert to string representation (hex)
    pub fn to_hex(&self) -> String {
        format!("{:04x}{}", self.shard_id, self.uuid.simple())
    }

    /// Null value (all zeros)
    pub const fn null() -> Self {
        Self {
            shard_id: 0,
            uuid: Uuid::nil(),
        }
    }

    /// Is empty
    pub fn is_null(&self) -> bool {
        self.shard_id == 0 && self.uuid.is_nil()
    }
}

impl Default for BeingId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for BeingId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "BeingId({})", self.to_hex())
    }
}

impl fmt::Display for BeingId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

impl Hash for BeingId {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.shard_id.hash(state);
        self.uuid.hash(state);
    }
}

impl FromStr for BeingId {
    type Err = crate::error::DaoQLError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.len() < 4 {
            return Err("BeingId string too short".into());
        }
        let shard_part = &s[0..4];
        let uuid_part = &s[4..];
        let shard_id = u16::from_str_radix(shard_part, 16)
            .map_err(|e| crate::error::DaoQLError::InvalidState(format!("shard_id parse failed: {e}")))?;
        let uuid = Uuid::parse_str(uuid_part)
            .map_err(|e| crate::error::DaoQLError::InvalidState(format!("UUID parse failed: {e}")))?;
        Ok(Self { shard_id, uuid })
    }
}

// ============================================================================
// Other identifiers
// ============================================================================

/// Define type encoding (edu edition uses u16, supporting 65536 types)
pub type DefTypeCode = u16;

/// Relation type encoding
pub type RelationTypeCode = u16;

/// Node offset in mmap file (bytes)
pub type NodeOffset = u64;

/// Edge offset in mmap file (bytes)
pub type EdgeOffset = u64;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_being_id_size() {
        assert_eq!(std::mem::size_of::<BeingId>(), 18);
    }

    #[test]
    fn test_being_id_roundtrip() {
        let id = BeingId::new();
        let bytes = id.to_bytes();
        let id2 = BeingId::from_bytes(&bytes).unwrap();
        assert_eq!(id, id2);
    }

    #[test]
    fn test_being_id_created_at() {
        let before = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        let id = BeingId::new();
        let after = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        let ts = id.created_at_ms();
        assert!(ts >= before && ts <= after);
    }

    #[test]
    fn test_being_id_hex_roundtrip() {
        let id = BeingId::new();
        let hex = id.to_hex();
        let id2 = BeingId::from_str(&hex).unwrap();
        assert_eq!(id, id2);
    }

    #[test]
    fn test_being_id_null() {
        let null = BeingId::null();
        assert!(null.is_null());
        let non_null = BeingId::new();
        assert!(!non_null.is_null());
    }
}
