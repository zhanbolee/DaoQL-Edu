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
//! PerBeingLock — 每个 Being 的细粒度锁
//!
//! 教学说明：
//! - 全局锁会导致严重并发瓶颈
//! - PerBeingLock 为每个 Being 分配一个 RwLock，实现细粒度并发控制
//! - 事务按 BeingId 排序后批量加锁，防止死锁
//! - 锁池使用 Arc<RwLock<()>>，锁对象在池中复用

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::error::{DaoQLError, TransactionError};
use crate::id::BeingId;

/// 每个 Being 的读写锁
///
/// 使用 std::sync::RwLock（非 tokio::sync，因为教学版是同步引擎）
pub type BeingLock = Arc<RwLock<()>>;

/// 锁管理器
///
/// 管理所有 Being 的锁对象，按需创建、惰性初始化。
#[derive(Clone)]
pub struct LockManager {
    /// BeingId → 锁对象的映射
    locks: Arc<RwLock<HashMap<BeingId, BeingLock>>>,
}

impl Default for LockManager {
    fn default() -> Self {
        Self::new()
    }
}

impl LockManager {
    pub fn new() -> Self {
        Self {
            locks: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 获取或创建锁对象
    fn get_or_create(&self, id: BeingId) -> BeingLock {
        // 先尝试读锁获取
        {
            let read_guard = self.locks.read().unwrap();
            if let Some(lock) = read_guard.get(&id) {
                return lock.clone();
            }
        }
        // 未找到，写锁创建
        let mut write_guard = self.locks.write().unwrap();
        write_guard.entry(id).or_insert_with(|| Arc::new(RwLock::new(()))).clone()
    }

    /// 获取单个 Being 的读锁对象
    ///
    /// 返回 Arc<RwLock<()>>，由调用者自行加锁
    pub fn read_lock(&self, id: BeingId) -> Result<BeingLock, DaoQLError> {
        let lock = self.get_or_create(id);
        // 预检查：尝试加读锁，确保锁可用
        let _guard = lock.read().map_err(|_| {
            DaoQLError::Transaction(TransactionError::LockTimeout(id))
        })?;
        drop(_guard);
        Ok(lock)
    }

    /// 获取单个 Being 的写锁对象
    ///
    /// 返回 Arc<RwLock<()>>，由调用者自行加锁
    pub fn write_lock(&self, id: BeingId) -> Result<BeingLock, DaoQLError> {
        let lock = self.get_or_create(id);
        // 预检查：尝试加写锁，确保锁可用
        let _guard = lock.write().map_err(|_| {
            DaoQLError::Transaction(TransactionError::LockTimeout(id))
        })?;
        drop(_guard);
        Ok(lock)
    }

    /// 批量获取写锁对象（按 BeingId 排序，防止死锁）
    ///
    /// 教学说明：
    /// - 死锁的必要条件之一是循环等待
    /// - 按全局顺序加锁，打破循环等待条件
    /// - 这是数据库中经典的死锁预防策略
    pub fn batch_write_lock(
        &self,
        mut ids: Vec<BeingId>,
    ) -> Result<Vec<BeingLock>, DaoQLError> {
        // 去重并排序
        ids.sort();
        ids.dedup();

        let mut locks = Vec::with_capacity(ids.len());
        for id in ids {
            locks.push(self.write_lock(id)?);
        }
        Ok(locks)
    }

    /// 锁数量（用于调试）
    pub fn lock_count(&self) -> usize {
        self.locks.read().unwrap().len()
    }
}

impl LockManager {
    /// 获取原始锁对象
    pub fn get_lock(&self, id: BeingId) -> BeingLock {
        self.get_or_create(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lock_manager_basic() {
        let lm = LockManager::new();
        let id = BeingId::new();

        let lock = lm.read_lock(id).unwrap();
        let _guard = lock.read().unwrap();
        drop(_guard);

        let lock = lm.write_lock(id).unwrap();
        let _guard = lock.write().unwrap();
        drop(_guard);
    }

    #[test]
    fn test_lock_manager_batch() {
        let lm = LockManager::new();
        let ids = vec![
            BeingId::new(),
            BeingId::new(),
            BeingId::new(),
        ];

        let locks = lm.batch_write_lock(ids.clone()).unwrap();
        assert_eq!(locks.len(), 3);
        let _guards: Vec<_> = locks.iter().map(|l| l.write().unwrap()).collect();
        drop(_guards);
    }

    #[test]
    fn test_lock_manager_dedup() {
        let lm = LockManager::new();
        let id = BeingId::new();
        let ids = vec![id, id, id];

        let locks = lm.batch_write_lock(ids).unwrap();
        assert_eq!(locks.len(), 1);
    }

    #[test]
    fn test_concurrent_reads() {
        let lm = Arc::new(LockManager::new());
        let id = BeingId::new();

        let mut handles = vec![];
        for _ in 0..10 {
            let lm = lm.clone();
            let handle = std::thread::spawn(move || {
                let lock = lm.read_lock(id).unwrap();
                let _guard = lock.read().unwrap();
                // 模拟读操作
                std::thread::sleep(std::time::Duration::from_millis(1));
                drop(_guard);
            });
            handles.push(handle);
        }

        for h in handles {
            h.join().unwrap();
        }
    }
}
