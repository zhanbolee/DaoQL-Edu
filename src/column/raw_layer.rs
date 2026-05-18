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
//! RawLayer — 原始行存储层
//!
//! 教学说明：
//! - append-only 设计：新记录追加到末尾，旧记录不修改
//! - 序列化：postcard（紧凑二进制）+ JSON（动态字段）
//! - 压缩：lz4_flex，减少磁盘占用
//! - 适合：文档查询、全字段检索、非结构化数据


use serde::{Deserialize, Serialize};

use crate::being::Being;
use crate::error::DaoQLError;
use crate::id::BeingId;

/// 原始记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawRecord {
    pub being_id: BeingId,
    pub def_type: String,
    pub timestamp: i64,
    /// postcard 序列化的 BeingCore
    pub core_data: Vec<u8>,
    /// JSON 序列化的动态字段
    pub ext_json: String,
}

/// RawLayer — 行存储
pub struct RawLayer {
    /// 内存缓冲区（未刷盘的记录）
    buffer: Vec<RawRecord>,
    /// 已刷盘的 chunk 列表
    chunks: Vec<RawChunk>,
    /// 总记录数
    total_records: u64,
}

/// 已持久化的 chunk
#[derive(Debug, Clone)]
pub struct RawChunk {
    pub chunk_id: u64,
    pub record_count: usize,
    pub compressed_size: usize,
}

impl RawLayer {
    pub fn new() -> Self {
        Self {
            buffer: Vec::with_capacity(1000),
            chunks: Vec::new(),
            total_records: 0,
        }
    }

    /// 追加 Being
    pub fn append(&mut self, being: &Being) -> Result<(), DaoQLError> {
        let record = RawRecord {
            being_id: being.core.id,
            def_type: being.core.def.clone(),
            timestamp: being.core.created_at,
            core_data: postcard::to_allocvec(&being.core)?,
            ext_json: match &being.ext {
                Some(ext) => serde_json::to_string(&ext.dynamic_attrs).unwrap_or_default(),
                None => "{}".to_string(),
            },
        };
        self.buffer.push(record);
        self.total_records += 1;
        Ok(())
    }

    /// 按 BeingId 查找（从缓冲区）
    pub fn find(&self, id: BeingId) -> Option<&RawRecord> {
        self.buffer.iter().find(|r| r.being_id == id)
    }

    /// 扫描缓冲区中的所有记录
    pub fn scan(&self) -> &[RawRecord] {
        &self.buffer
    }

    /// 刷盘（教学版简化：返回序列化数据，实际写入由上层管理）
    pub fn flush(&mut self) -> Result<Vec<u8>, DaoQLError> {
        if self.buffer.is_empty() {
            return Ok(Vec::new());
        }

        let data = postcard::to_allocvec(&self.buffer)?;
        let compressed = lz4_flex::compress(&data);

        let chunk = RawChunk {
            chunk_id: self.chunks.len() as u64,
            record_count: self.buffer.len(),
            compressed_size: compressed.len(),
        };
        self.chunks.push(chunk);
        self.buffer.clear();

        Ok(compressed)
    }

    /// 总记录数
    pub fn total_records(&self) -> u64 {
        self.total_records
    }

    /// chunk 数量
    pub fn chunk_count(&self) -> usize {
        self.chunks.len()
    }
}

impl Default for RawLayer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_raw_layer_append() {
        let mut layer = RawLayer::new();
        let being = Being::new("Test", "Def").with_attr("x", serde_json::json!(1));

        layer.append(&being).unwrap();
        assert_eq!(layer.total_records(), 1);

        let found = layer.find(being.core.id).unwrap();
        assert_eq!(found.being_id, being.core.id);
    }

    #[test]
    fn test_raw_layer_flush() {
        let mut layer = RawLayer::new();
        for i in 0..100 {
            let being = Being::new(format!("Item{}", i), "ItemDef");
            layer.append(&being).unwrap();
        }

        let compressed = layer.flush().unwrap();
        assert!(!compressed.is_empty());
        assert_eq!(layer.chunk_count(), 1);
        assert_eq!(layer.total_records(), 100);
    }
}
