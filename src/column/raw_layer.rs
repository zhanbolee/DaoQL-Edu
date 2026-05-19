// Copyright (c) 2026 Zhanbo Li / Atlas Lee <4859345@qq.com>
// SPDX-License-Identifier: AGPL-3.0-or-later
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published
// by the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.


use serde::{Deserialize, Serialize};

use crate::being::Being;
use crate::error::DaoQLError;
use crate::id::BeingId;

/// Rawrecord
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawRecord {
    pub being_id: BeingId,
    pub def_type: String,
    pub timestamp: i64,
    /// postcard serialized BeingCore
    pub core_data: Vec<u8>,
    /// JSON serializeddynamicfield
    pub ext_json: String,
}

/// RawLayer — row store
pub struct RawLayer {
    /// Memory buffer (unflushed records)
    buffer: Vec<RawRecord>,
    /// AlreadyFlush chunk list
    chunks: Vec<RawChunk>,
    /// Total record count
    total_records: u64,
}

/// Already persisted chunk
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

    /// append Being
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

    /// Find by BeingId (secondary buffer)
    pub fn find(&self, id: BeingId) -> Option<&RawRecord> {
        self.buffer.iter().find(|r| r.being_id == id)
    }

    /// Scan buffer all records
    pub fn scan(&self) -> &[RawRecord] {
        &self.buffer
    }

    /// Flush (edu edition simplification: return serialized data, write managed by upper layer)
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

    /// Total record count
    pub fn total_records(&self) -> u64 {
        self.total_records
    }

    /// chunk quantity
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
