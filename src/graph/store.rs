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
//! Graph Storage Engine
//!
//! Educational Notes:
//! - Based on mmap fixed-length records (NodeRecord/EdgeRecord)
//! - Index-free adjacency: edges inline-linked via next_out/next_in pointers
//! - Update = Create new version，oldversionreserve（MVCC）
//! - all unsafe conversions go through boundary checks

use std::sync::Arc;

use crate::being::BeingCore;
use crate::error::DaoQLError;
use crate::graph::lock::LockManager;
use crate::graph::record::{EdgeRecord, NodeRecord};
use crate::id::{BeingId, NodeOffset};
use crate::relation::Relation;
use crate::storage::StorageManager;
use crate::transaction::TxId;

/// Graph storageengine
#[derive(Clone)]
pub struct GraphStore {
    /// Storage manager
    pub storage: Arc<StorageManager>,
    /// Lockmanager
    pub locks: LockManager,
    /// Current transaction number（used for MVCC）
    pub current_tx: TxId,
}

impl GraphStore {
    pub fn new(storage: Arc<StorageManager>) -> Self {
        Self {
            storage,
            locks: LockManager::new(),
            current_tx: 0,
        }
    }

    /// Get mutable storage reference (edu edition: single-thread safe)
    ///
    /// # SAFETY
    /// - only used single-threaded
    /// - caller guarantees no concurrent access
    unsafe fn storage_mut(&mut self) -> &mut StorageManager {
        let ptr = Arc::as_ptr(&self.storage);
        &mut *(ptr as *mut StorageManager)
    }

    /// Set current transaction number
    pub fn set_tx(&mut self, tx_id: TxId) {
        self.current_tx = tx_id;
    }

    /// Create node, return offset
    ///
    /// Flow：
    /// 1. allocate NodeRecord
    /// 2. filling BeingCore field
    /// 3. return offset (subsequently used for index)
    pub fn create_node(&mut self, core: &BeingCore, def_type_code: u16) -> Result<NodeOffset, DaoQLError> {
        let offset = unsafe { self.storage_mut() }.alloc_node()?;
        let idx = (offset / NodeRecord::SIZE as u64) as usize;

        {
            let buf = unsafe { self.storage_mut() }.nodes.get_mut(idx)?;
            // SAFETY: buf length = NodeRecord::SIZE, index range checked
            let rec = unsafe { &mut *(buf.as_mut_ptr() as *mut NodeRecord) };
            *rec = NodeRecord::new(core.id);
            rec.def_type_code = def_type_code;
            rec.set_def_name(&core.def);
            rec.status = core.status;
            rec.set_name(&core.name);
            rec.set_code(&core.code);
            rec.set_description(&core.description);
            rec.weight = core.weight;
            rec.priority = core.priority;
            rec.set_category(&core.category);
            rec.created_at = core.created_at;
            rec.updated_at = core.updated_at;
            rec.tx_begin = self.current_tx;
            rec.tx_end = u64::MAX;
        }

        Ok(offset)
    }

    /// Read node（read-only）
    pub fn read_node(&self, offset: NodeOffset) -> Result<&NodeRecord, DaoQLError> {
        let idx = (offset / NodeRecord::SIZE as u64) as usize;
        let buf = self.storage.nodes.get(idx)?;
        // SAFETY: buf length correct, index checked
        let rec = unsafe { &*(buf.as_ptr() as *const NodeRecord) };
        Ok(rec)
    }

    /// Read node（mutable）
    pub fn read_node_mut(&mut self, offset: NodeOffset) -> Result<&mut NodeRecord, DaoQLError> {
        let idx = (offset / NodeRecord::SIZE as u64) as usize;
        let buf = unsafe { self.storage_mut() }.nodes.get_mut(idx)?;
        // SAFETY: buf length correct, index checked，&mut self guarantee exclusive
        let rec = unsafe { &mut *(buf.as_mut_ptr() as *mut NodeRecord) };
        Ok(rec)
    }

    /// Create edge
    ///
    /// Flow：
    /// 1. allocate EdgeRecord
    /// 2. filling from_id/to_id/relation_type
    /// 3. link to from Nodeoutgoing edge linked list
    /// 4. link to to Nodeincoming edge linked list
    pub fn create_edge(&mut self, relation: &Relation) -> Result<NodeOffset, DaoQLError> {
        let edge_offset = unsafe { self.storage_mut() }.alloc_edge()?;
        let edge_idx = (edge_offset / EdgeRecord::SIZE as u64) as usize;

        // get from/to Node offset
        // edu edition simplification: assume offset already mapped from BeingId
        // actual should be secondary index level query
        let from_offset = self.find_node_offset(relation.from_id)?;
        let to_offset = self.find_node_offset(relation.to_id)?;

        {
            let buf = unsafe { self.storage_mut() }.edges.get_mut(edge_idx)?;
            let rec = unsafe { &mut *(buf.as_mut_ptr() as *mut EdgeRecord) };
            *rec = EdgeRecord::empty();
            rec.from_id = relation.from_id;
            rec.to_id = relation.to_id;
            rec.relation_type = relation.relation_type;
            rec.directed = if relation.directed { 1 } else { 0 };
            rec.created_at = relation.created_at;
            rec.tx_begin = self.current_tx;
            rec.set_name(&relation.name);
            rec.weight = relation.weight;
        }

        // link to from Nodeoutgoing edge linked list（head insertion）
        self.link_out_edge(from_offset, edge_offset)?;

        // link to to Nodeincoming edge linked list
        if relation.directed {
            self.link_in_edge(to_offset, edge_offset)?;
        } else {
            // undirected edge: bidirectional link
            self.link_out_edge(to_offset, edge_offset)?;
            self.link_in_edge(to_offset, edge_offset)?;
            // reverse also needed
            self.link_in_edge(from_offset, edge_offset)?;
        }

        Ok(edge_offset)
    }

    /// Link outgoing edge（head insertion）
    fn link_out_edge(
        &mut self,
        node_offset: NodeOffset,
        edge_offset: NodeOffset,
    ) -> Result<(), DaoQLError> {
        let node = self.read_node_mut(node_offset)?;
        let old_first = node.first_out_edge_offset;
        node.first_out_edge_offset = edge_offset;

        // Updatenewedge next_out
        let edge_idx = (edge_offset / EdgeRecord::SIZE as u64) as usize;
        let buf = unsafe { self.storage_mut() }.edges.get_mut(edge_idx)?;
        let edge = unsafe { &mut *(buf.as_mut_ptr() as *mut EdgeRecord) };
        edge.next_out_edge_offset = old_first;

        Ok(())
    }

    /// Link incoming edge（head insertion）
    fn link_in_edge(
        &mut self,
        node_offset: NodeOffset,
        edge_offset: NodeOffset,
    ) -> Result<(), DaoQLError> {
        let node = self.read_node_mut(node_offset)?;
        let old_first = node.first_in_edge_offset;
        node.first_in_edge_offset = edge_offset;

        let edge_idx = (edge_offset / EdgeRecord::SIZE as u64) as usize;
        let buf = unsafe { self.storage_mut() }.edges.get_mut(edge_idx)?;
        let edge = unsafe { &mut *(buf.as_mut_ptr() as *mut EdgeRecord) };
        edge.next_in_edge_offset = old_first;

        Ok(())
    }

    /// Get all outgoing edges
    pub fn out_edges(&self, node_offset: NodeOffset) -> Result<Vec<&EdgeRecord>, DaoQLError> {
        let node = self.read_node(node_offset)?;
        let mut edges = Vec::new();
        let mut current = node.first_out_edge_offset;

        while current != u64::MAX {
            let idx = (current / EdgeRecord::SIZE as u64) as usize;
            let buf = self.storage.edges.get(idx)?;
            let edge = unsafe { &*(buf.as_ptr() as *const EdgeRecord) };
            if !edge.is_empty() {
                edges.push(edge);
            }
            current = edge.next_out_edge_offset;
        }

        Ok(edges)
    }

    /// Get all incoming edges
    pub fn in_edges(&self, node_offset: NodeOffset) -> Result<Vec<&EdgeRecord>, DaoQLError> {
        let node = self.read_node(node_offset)?;
        let mut edges = Vec::new();
        let mut current = node.first_in_edge_offset;

        while current != u64::MAX {
            let idx = (current / EdgeRecord::SIZE as u64) as usize;
            let buf = self.storage.edges.get(idx)?;
            let edge = unsafe { &*(buf.as_ptr() as *const EdgeRecord) };
            if !edge.is_empty() {
                edges.push(edge);
            }
            current = edge.next_in_edge_offset;
        }

        Ok(edges)
    }

    /// Find node offset by BeingId (edu edition: linear scan)
    ///
    /// Production version should use UUID→Offset index。
    pub fn find_node_offset(&self, id: BeingId) -> Result<NodeOffset, DaoQLError> {
        for i in 0..self.storage.node_count() {
            let buf = self.storage.nodes.get(i)?;
            let rec = unsafe { &*(buf.as_ptr() as *const NodeRecord) };
            if rec.id == id {
                return Ok((i * NodeRecord::SIZE) as u64);
            }
        }
        Err(DaoQLError::NotFound(id))
    }

    /// GetNode count
    pub fn node_count(&self) -> usize {
        self.storage.node_count()
    }

    /// GetEdge count
    pub fn edge_count(&self) -> usize {
        self.storage.edge_count()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::config::Config;

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join("daoql-edu-test-graph").join(name);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn create_test_store(dir: &std::path::Path) -> GraphStore {
        let storage = Arc::new(StorageManager::open(dir, Config::default()).unwrap());
        GraphStore::new(storage)
    }

    #[test]
    fn test_create_and_read_node() {
        let dir = temp_dir("node_rw");
        let mut store = create_test_store(&dir);
        store.set_tx(1);

        let core = BeingCore {
            id: BeingId::new(),
            def: "Test".to_string(),
            status: 1,
            name: "Test Node".to_string(),
            code: "TN001".to_string(),
            description: "A test node".to_string(),
            weight: 1.5,
            priority: 10,
            category: "test".to_string(),
            created_at: 1234567890,
            updated_at: 1234567890,
            tx_begin: 1,
            tx_end: u64::MAX,
        };

        let offset = store.create_node(&core, 0).unwrap();
        let node = store.read_node(offset).unwrap();

        assert_eq!(node.id, core.id);
        assert_eq!(node.status, 1);
        assert_eq!(node.get_name(), "Test Node");
        assert_eq!(node.get_code(), "TN001");
        assert_eq!(node.weight, 1.5);
    }

    #[test]
    fn test_create_and_read_edge() {
        let dir = temp_dir("edge_rw");
        let mut store = create_test_store(&dir);
        store.set_tx(1);

        let id1 = BeingId::new();
        let id2 = BeingId::new();

        let core1 = BeingCore {
            id: id1,
            def: "A".to_string(),
            status: 0,
            name: "A".to_string(),
            code: "".to_string(),
            description: "".to_string(),
            weight: 0.0,
            priority: 0,
            category: "".to_string(),
            created_at: 0,
            updated_at: 0,
            tx_begin: 1,
            tx_end: u64::MAX,
        };
        let core2 = BeingCore {
            id: id2,
            def: "B".to_string(),
            status: 0,
            name: "B".to_string(),
            code: "".to_string(),
            description: "".to_string(),
            weight: 0.0,
            priority: 0,
            category: "".to_string(),
            created_at: 0,
            updated_at: 0,
            tx_begin: 1,
            tx_end: u64::MAX,
        };

        let off1 = store.create_node(&core1, 0).unwrap();
        let _off2 = store.create_node(&core2, 0).unwrap();

        let rel = Relation::new(id1, id2, 1, true).with_name("test_rel");
        let edge_off = store.create_edge(&rel).unwrap();

        let node1 = store.read_node(off1).unwrap();
        assert_eq!(node1.first_out_edge_offset, edge_off, "first_out_edge_offset should be set");

        let out_edges = store.out_edges(off1).unwrap();
        for e in &out_edges {
            println!("DEBUG: out_edge from_id={:?} to_id={:?} is_empty={}", e.from_id, e.to_id, e.is_empty());
        }
        assert_eq!(out_edges.len(), 1, "expected 1 out edge, got {}", out_edges.len());
        assert_eq!(out_edges[0].to_id, id2);
    }

    #[test]
    fn test_node_count() {
        let dir = temp_dir("count");
        let mut store = create_test_store(&dir);
        store.set_tx(1);

        assert_eq!(store.node_count(), 0);

        for i in 0..5 {
            let core = BeingCore {
                id: BeingId::new(),
                def: "Test".to_string(),
                status: 0,
                name: format!("Node {}", i),
                code: "".to_string(),
                description: "".to_string(),
                weight: 0.0,
                priority: 0,
                category: "".to_string(),
                created_at: 0,
                updated_at: 0,
                tx_begin: 1,
                tx_end: u64::MAX,
            };
            store.create_node(&core, 0).unwrap();
        }

        assert_eq!(store.node_count(), 5);
    }
}
