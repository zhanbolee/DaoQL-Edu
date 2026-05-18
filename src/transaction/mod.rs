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
//! 事务管理器
//!
//! 教学说明：
//! - Read Committed 隔离级别
//! - PerBeingLock 细粒度锁
//! - 两阶段提交：排序加锁 → 执行 → WAL → 释放

pub mod committer;
pub mod lock_manager;
pub mod validator;

pub use committer::Transaction;
pub use lock_manager::LockManager;
pub use validator::ConstraintValidator;

/// 事务 ID（单调递增）
pub type TxId = u64;

/// 活跃事务集合
pub struct ActiveTxSet {
    pub active: std::sync::RwLock<std::collections::BTreeSet<TxId>>,
}

impl ActiveTxSet {
    pub fn new() -> Self {
        Self {
            active: std::sync::RwLock::new(std::collections::BTreeSet::new()),
        }
    }

    pub fn add(&self, tx_id: TxId) {
        self.active.write().unwrap().insert(tx_id);
    }

    pub fn remove(&self, tx_id: TxId) {
        self.active.write().unwrap().remove(&tx_id);
    }

    pub fn is_active(&self, tx_id: TxId) -> bool {
        self.active.read().unwrap().contains(&tx_id)
    }

    pub fn list(&self) -> Vec<TxId> {
        self.active.read().unwrap().iter().copied().collect()
    }
}

impl Default for ActiveTxSet {
    fn default() -> Self {
        Self::new()
    }
}
