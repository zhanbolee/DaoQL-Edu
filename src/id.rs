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
//! 全局标识符系统
//!
//! 教学说明：
//! - BeingId = shard_id(2B) + UUID v7(16B) = 18B
//! - UUID v7 是时间排序的 UUID，使按时间顺序的插入天然有序
//! - shard_id 预留分片扩展（教学版固定为 0）
//! - 定长设计便于 mmap 内联存储

use std::fmt;
use std::hash::{Hash, Hasher};
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 全局唯一实体标识符
///
/// 内存布局（18 bytes）：
/// ```ignore
/// [shard_id: u16 | 2 bytes][uuid: Uuid | 16 bytes]
/// ```ignore
///
/// 使用 UUID v7（时间排序 UUID）的好处：
/// - 时间戳在高位，按创建时间排序无需额外索引
/// - 兼容标准 UUID 工具链
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct BeingId {
    pub shard_id: u16,
    pub uuid: Uuid,
}

impl BeingId {
    /// 固定大小 18 bytes
    pub const SIZE: usize = 18;

    /// 创建新的 BeingId（shard_id 固定为 0，教学版）
    pub fn new() -> Self {
        Self {
            shard_id: 0,
            uuid: Uuid::now_v7(),
        }
    }

    /// 从 shard_id + UUID 创建
    pub fn from_parts(shard_id: u16, uuid: Uuid) -> Self {
        Self { shard_id, uuid }
    }

    /// 序列化为 18 字节数组
    pub fn to_bytes(&self) -> [u8; 18] {
        let mut buf = [0u8; 18];
        buf[0..2].copy_from_slice(&self.shard_id.to_be_bytes());
        buf[2..18].copy_from_slice(self.uuid.as_bytes());
        buf
    }

    /// 从 18 字节数组解析
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != 18 {
            return None;
        }
        let shard_id = u16::from_be_bytes([bytes[0], bytes[1]]);
        let uuid = Uuid::from_slice(&bytes[2..18]).ok()?;
        Some(Self { shard_id, uuid })
    }

    /// 获取创建时间（从 UUID v7 提取）
    ///
    /// UUID v7 格式：
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

    /// 转换为字符串表示（hex）
    pub fn to_hex(&self) -> String {
        format!("{:04x}{}", self.shard_id, self.uuid.simple())
    }

    /// 空值（全零）
    pub const fn null() -> Self {
        Self {
            shard_id: 0,
            uuid: Uuid::nil(),
        }
    }

    /// 是否为空
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
            return Err("BeingId 字符串过短".into());
        }
        let shard_part = &s[0..4];
        let uuid_part = &s[4..];
        let shard_id = u16::from_str_radix(shard_part, 16)
            .map_err(|e| crate::error::DaoQLError::InvalidState(format!("shard_id 解析失败: {e}")))?;
        let uuid = Uuid::parse_str(uuid_part)
            .map_err(|e| crate::error::DaoQLError::InvalidState(format!("UUID 解析失败: {e}")))?;
        Ok(Self { shard_id, uuid })
    }
}

// ============================================================================
// 其他标识符
// ============================================================================

/// 定义类型编码（教学版使用 u16，支持 65536 种类型）
pub type DefTypeCode = u16;

/// 关系类型编码
pub type RelationTypeCode = u16;

/// 节点在 mmap 文件中的偏移量（字节）
pub type NodeOffset = u64;

/// 边在 mmap 文件中的偏移量（字节）
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
