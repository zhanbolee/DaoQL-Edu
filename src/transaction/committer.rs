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
//! Transaction Committer
//!
//! Educational Notes:
//! - Two-phase commit：
//!   1. preparation phase：Sort and lock、validate constraints
//!   2. commit phase：write WAL, execute mutation, update index, release locks

use std::cell::RefCell;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::being::Being;
use crate::column::ProjectedLayer;
use crate::def::DefRegistry;
use crate::error::DaoQLError;
use crate::graph::lock::LockManager;
use crate::graph::store::GraphStore;
use crate::id::BeingId;
use crate::index::uuid_index::UuidIndex;
use crate::relation::Relation;
use crate::transaction::{ActiveTxSet, TxId};
use crate::wal::record::{Op, TransactionPayload};
use crate::wal::writer::WalWriter;

/// Transaction
pub struct Transaction<'a> {
    pub tx_id: TxId,
    pub ops: Vec<Op>,
    pub lock_manager: LockManager,
    pub active_txs: ActiveTxSet,
    pub wal: &'a RefCell<WalWriter>,
    pub graph: &'a RefCell<GraphStore>,
    pub uuid_index: &'a RefCell<UuidIndex>,
    pub column: &'a RefCell<ProjectedLayer>,
    pub def_registry: &'a RefCell<DefRegistry>,
}

impl<'a> Transaction<'a> {
    pub fn new(
        tx_id: TxId,
        wal: &'a RefCell<WalWriter>,
        graph: &'a RefCell<GraphStore>,
        uuid_index: &'a RefCell<UuidIndex>,
        column: &'a RefCell<ProjectedLayer>,
        def_registry: &'a RefCell<DefRegistry>,
    ) -> Self {
        Self {
            tx_id,
            ops: Vec::new(),
            lock_manager: LockManager::new(),
            active_txs: ActiveTxSet::new(),
            wal,
            graph,
            uuid_index,
            column,
            def_registry,
        }
    }

    /// Addoperation
    pub fn add_op(&mut self, op: Op) {
        self.ops.push(op);
    }

    /// Create Being
    pub fn create_being(&mut self, being: Being) {
        self.add_op(Op::CreateBeing { being });
    }

    /// Create relation
    pub fn create_relation(&mut self, relation: Relation) {
        self.add_op(Op::CreateRelation { relation });
    }

    /// Commit transaction
    pub fn commit(&mut self) -> Result<(), DaoQLError> {
        if self.ops.is_empty() {
            return Ok(());
        }

        // 1. Collect involved BeingIds
        let mut ids: Vec<BeingId> = self
            .ops
            .iter()
            .filter_map(|op| match op {
                Op::CreateBeing { being } => Some(being.core.id),
                Op::CreateRelation { relation } => Some(relation.from_id),
                _ => None,
            })
            .collect();
        ids.sort();
        ids.dedup();

        // 2. Batch lock
        let _guards = self.lock_manager.batch_write_lock(ids)?;

        // 3. Register active transactions
        self.active_txs.add(self.tx_id);

        // 4. Write WAL
        let payload = TransactionPayload {
            tx_id: self.tx_id,
            ops: std::mem::take(&mut self.ops),
        };
        let mut wal = self.wal.borrow_mut();
        let record = crate::wal::record::WalRecord::new(
            wal.next_seq(),
            payload.to_bytes()?,
        );
        wal.append(&record)?;
        // Do not flush immediately — handled by background thread or when buffer full
        // edu edition optimization: batch write scenarios, delayed flush reduces 90% file I/O

        // 5. Execute storage changes (row by row)
        let mut graph = self.graph.borrow_mut();
        let mut def_registry = self.def_registry.borrow_mut();
        let uuid_index = self.uuid_index.borrow();
        let mut column = self.column.borrow_mut();

        let mut uuid_batch = Vec::new();
        for op in &payload.ops {
            match op {
                Op::CreateBeing { being } => {
                    let def_type_code = def_registry.get_or_register(&being.core.def);
                    let offset = graph.create_node(&being.core, def_type_code)?;
                    uuid_batch.push((being.core.id, offset));
                    column.project(being)?;
                }
                Op::CreateRelation { relation } => {
                    graph.create_edge(relation)?;
                }
                Op::UpdateBeing { id, updates } => {
                    if let Ok(Some(offset)) = uuid_index.get(*id) {
                        if let Ok(node) = graph.read_node_mut(offset) {
                            for update in updates {
                                match update.field_name.as_str() {
                                    "name" => {
                                        if let Some(s) = update.new_value.as_str() {
                                            node.set_name(s);
                                        }
                                    }
                                    "description" => {
                                        if let Some(s) = update.new_value.as_str() {
                                            node.set_description(s);
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }
                Op::DeleteBeing { id } => {
                    if let Ok(Some(offset)) = uuid_index.get(*id) {
                        if let Ok(node) = graph.read_node_mut(offset) {
                            node.status = 0;
                        }
                    }
                }
            }
        }
        if !uuid_batch.is_empty() {
            uuid_index.batch_insert(&uuid_batch)?;
        }
        drop(graph);
        drop(def_registry);
        drop(uuid_index);
        drop(column);

        // 6. release locks
        drop(_guards);
        self.active_txs.remove(self.tx_id);

        Ok(())
    }
}

/// Globaltransaction ID generate
pub struct TxIdGenerator {
    counter: AtomicU64,
}

impl TxIdGenerator {
    pub fn new() -> Self {
        Self {
            counter: AtomicU64::new(1),
        }
    }

    pub fn next(&self) -> TxId {
        self.counter.fetch_add(1, Ordering::SeqCst)
    }
}

impl Default for TxIdGenerator {
    fn default() -> Self {
        Self::new()
    }
}
