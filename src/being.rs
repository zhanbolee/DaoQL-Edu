// Copyright (c) 2026 Zhanbo Li / Atlas Lee <4859345@qq.com>
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
//! Being (Entity) — everything that exists in the world
//!
//! Educational Notes:
//! - Being is the core concept of DaoQL, representing everything that can be identified
//! - Composed of three parts: BeingCore (fixed fields) + BeingExt (dynamic fields) + embeddings (vectors)
//! - When writing, split into NodeRecord (mmap fixed-length) and external storage (ext/embedding)
//! - This 'fixed+dynamic' design balances performance (fixed fields inline) and flexibility (dynamic fields without schema constraints)

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::DaoQLError;
use crate::id::BeingId;

/// In-memory full Being representation
///
/// This is the user-facing data structure, assembled from NodeRecord + ext + embedding.
/// When writing, split into multiple parts for separate storage.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Being {
    /// Fixed core fields
    pub core: BeingCore,
    /// Dynamic extended attributes (optional)
    pub ext: Option<BeingExt>,
    /// Embedding vectors (field name → vector)
    pub embeddings: HashMap<String, Vec<f32>>,
}

impl Being {
    /// Create new Being (auto-generates BeingId)
    pub fn new(name: impl Into<String>, def: impl Into<String>) -> Self {
        Self {
            core: BeingCore::new(name, def),
            ext: None,
            embeddings: HashMap::new(),
        }
    }

    /// Create from BeingId (used for reconstruction after reading)
    pub fn with_id(id: BeingId) -> Self {
        Self {
            core: BeingCore::with_id(id),
            ext: None,
            embeddings: HashMap::new(),
        }
    }

    /// Reconstruct Being from NodeRecord
    pub fn from_node(node: &crate::graph::record::NodeRecord) -> Result<Self, DaoQLError> {
        Ok(Self {
            core: BeingCore {
                id: node.id,
                def: node.get_def_name(),
                status: node.status,
                name: node.get_name(),
                code: node.get_code(),
                description: node.get_description(),
                weight: node.weight,
                priority: node.priority,
                category: node.get_category(),
                created_at: node.created_at,
                updated_at: node.updated_at,
                tx_begin: node.tx_begin,
                tx_end: node.tx_end,
            },
            ext: None,
            embeddings: HashMap::new(),
        })
    }

    /// Add dynamic field
    pub fn with_attr(mut self, key: impl Into<String>, value: Value) -> Self {
        self.ext.get_or_insert_with(|| BeingExt {
            being_id: self.core.id,
            timestamp: self.core.updated_at,
            dynamic_attrs: HashMap::new(),
        });
        if let Some(ref mut ext) = self.ext {
            ext.dynamic_attrs.insert(key.into(), value);
        }
        self
    }

    /// Add embedding vector
    pub fn with_embedding(mut self, field: impl Into<String>, vector: Vec<f32>) -> Self {
        self.embeddings.insert(field.into(), vector);
        self
    }

    /// Get dynamic field value
    pub fn attr(&self, key: &str) -> Option<&Value> {
        self.ext.as_ref()?.dynamic_attrs.get(key)
    }

    /// Get embedding vector
    pub fn embedding(&self, field: &str) -> Option<&Vec<f32>> {
        self.embeddings.get(field)
    }

    /// Update status
    pub fn set_status(&mut self, status: u8) {
        self.core.status = status;
        self.core.updated_at = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
    }

    /// Validate Being integrity
    pub fn validate(&self) -> Result<(), DaoQLError> {
        if self.core.name.is_empty() {
            return Err(DaoQLError::ConstraintViolation(
                "Being name cannot be empty".to_string(),
            ));
        }
        if self.core.def.is_empty() {
            return Err(DaoQLError::ConstraintViolation(
                "Being def cannot be empty".to_string(),
            ));
        }
        // Validate vector dimension consistency
        for (field, vec) in &self.embeddings {
            if vec.is_empty() {
                return Err(DaoQLError::ConstraintViolation(format!(
                    "Embedding vector {field} cannot be empty"
                )));
            }
        }
        Ok(())
    }
}

/// Being Fixed core fields
///
/// These fields are inline-stored in NodeRecord (1536 bytes), enabling O(1) random access.
/// Edu edition contains ~20 common fields; production may have more.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BeingCore {
    /// Globally unique identifier
    pub id: BeingId,
    /// Type definition name (e.g. "Order")
    pub def: String,
    /// Status code (0 = normal, 1 = disabled, etc.)
    pub status: u8,
    /// Display name
    pub name: String,
    /// Business code
    pub code: String,
    /// Description
    pub description: String,
    /// Weight (for sorting/priority)
    pub weight: f64,
    /// Priority
    pub priority: i32,
    /// Category
    pub category: String,
    /// Creation time (nanosecond timestamp)
    pub created_at: i64,
    /// Update time (nanosecond timestamp)
    pub updated_at: i64,
    /// MVCC begin transaction number
    pub tx_begin: u64,
    /// MVCC end transaction number (u64::MAX = active)
    pub tx_end: u64,
}

impl BeingCore {
    /// Create new BeingCore
    pub fn new(name: impl Into<String>, def: impl Into<String>) -> Self {
        let now = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
        Self {
            id: BeingId::new(),
            def: def.into(),
            status: 0,
            name: name.into(),
            code: String::new(),
            description: String::new(),
            weight: 0.0,
            priority: 0,
            category: String::new(),
            created_at: now,
            updated_at: now,
            tx_begin: 0,
            tx_end: u64::MAX,
        }
    }

    /// Create from BeingId (used for reconstruction after reading)
    pub fn with_id(id: BeingId) -> Self {
        let now = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
        Self {
            id,
            def: String::new(),
            status: 0,
            name: String::new(),
            code: String::new(),
            description: String::new(),
            weight: 0.0,
            priority: 0,
            category: String::new(),
            created_at: now,
            updated_at: now,
            tx_begin: 0,
            tx_end: u64::MAX,
        }
    }

    /// Determine visibility under current transaction
    ///
    /// MVCC visibility rules (Read Committed):
    /// - Committed: tx_begin ≤ current_txn AND (tx_end > current_txn OR tx_end = u64::MAX)
    /// - Uncommitted: tx_begin is in the current active transaction set
    pub fn is_visible(&self, tx_id: u64, active_txs: &[u64]) -> bool {
        // Self-created in current transaction, always visible
        if self.tx_begin == tx_id {
            return true;
        }
        // Creator committed (tx_begin not in active set)
        let creator_committed = !active_txs.contains(&self.tx_begin);
        // Current version ended (updated/deleted)
        let is_ended = self.tx_end != u64::MAX;
        // End-er committed
        let ender_committed = is_ended && !active_txs.contains(&self.tx_end);

        // Visibility: creator committed, and (not ended OR ender committed)
        creator_committed && (!is_ended || ender_committed)
    }
}

/// Being dynamic extended attributes
///
/// Key-value storage without schema constraints.
/// Stored in external file (non-mmap fixed-length area), referenced via NodeRecord.ext_offset.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BeingExt {
    /// Associated BeingId
    pub being_id: BeingId,
    /// Extended attribute timestamp
    pub timestamp: i64,
    /// Dynamic attribute map (key → JSON value)
    pub dynamic_attrs: HashMap<String, Value>,
}

impl BeingExt {
    /// Create empty extension
    pub fn new(being_id: BeingId) -> Self {
        Self {
            being_id,
            timestamp: chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0),
            dynamic_attrs: HashMap::new(),
        }
    }

    /// Set dynamic field
    pub fn set(&mut self, key: impl Into<String>, value: Value) {
        self.dynamic_attrs.insert(key.into(), value);
        self.timestamp = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
    }

    /// Get dynamic field
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.dynamic_attrs.get(key)
    }

    /// Serialize to JSON
    pub fn to_bytes(&self) -> Result<Vec<u8>, DaoQLError> {
        Ok(serde_json::to_vec(self)?)
    }

    /// Deserialize from JSON
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, DaoQLError> {
        Ok(serde_json::from_slice(bytes)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_being_creation() {
        let b = Being::new("Test Being", "TestDef");
        assert_eq!(b.core.name, "Test Being");
        assert_eq!(b.core.def, "TestDef");
        assert_eq!(b.core.status, 0);
        assert!(b.ext.is_none());
        assert!(b.embeddings.is_empty());
    }

    #[test]
    fn test_being_with_attr() {
        let b = Being::new("Product", "ProductDef")
            .with_attr("price", serde_json::json!(99.99))
            .with_attr("count", serde_json::json!(100));

        assert_eq!(b.attr("price"), Some(&serde_json::json!(99.99)));
        assert_eq!(b.attr("count"), Some(&serde_json::json!(100)));
        assert_eq!(b.attr("missing"), None);
    }

    #[test]
    fn test_being_with_embedding() {
        let emb = vec![0.1_f32, 0.2, 0.3];
        let b = Being::new("Doc", "DocDef").with_embedding("text_emb", emb.clone());
        assert_eq!(b.embedding("text_emb"), Some(&emb));
        assert_eq!(b.embedding("missing"), None);
    }

    #[test]
    fn test_being_validate() {
        let b = Being::new("", "Def");
        assert!(b.validate().is_err());

        let b = Being::new("Name", "");
        assert!(b.validate().is_err());

        let b = Being::new("Name", "Def");
        assert!(b.validate().is_ok());
    }

    #[test]
    fn test_being_core_visibility() {
        let mut core = Being::new("Test", "Def").core;
        core.tx_begin = 10;
        core.tx_end = u64::MAX;

        // Txn 20 reads: creator 10 committed (not in active set), not ended → visible
        assert!(core.is_visible(20, &[]));

        // Txn 20 reads: creator 10 still active → not visible
        assert!(!core.is_visible(20, &[10]));

        // Txn 20 reads: ended (tx_end=15), end-er committed → visible (old version)
        core.tx_end = 15;
        assert!(core.is_visible(20, &[]));

        // Txn 20 reads: ended, end-er not committed → not visible
        assert!(!core.is_visible(20, &[15]));

        // Self-created transactions are always visible
        assert!(core.is_visible(10, &[]));
    }

    #[test]
    fn test_being_ext_roundtrip() {
        let id = BeingId::new();
        let mut ext = BeingExt::new(id);
        ext.set("color", serde_json::json!("red"));
        ext.set("size", serde_json::json!(42));

        let bytes = ext.to_bytes().unwrap();
        let ext2 = BeingExt::from_bytes(&bytes).unwrap();
        assert_eq!(ext.dynamic_attrs, ext2.dynamic_attrs);
    }
}
