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

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::error::{DaoQLError, TransactionError};
use crate::id::BeingId;

/// each Being read-write lock
///
/// Use std::sync::RwLock（not tokio::sync, because edu edition is synchronous engine）
pub type BeingLock = Arc<RwLock<()>>;

/// Lockmanager
///
/// Manage all Being lock objects, create on demand, lazy initialize。
#[derive(Clone)]
pub struct LockManager {
    /// BeingId → lock objectmap
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

    /// GetOrCreatelockobject
    fn get_or_create(&self, id: BeingId) -> BeingLock {
        // Try read lock first
        {
            let read_guard = self.locks.read().unwrap();
            if let Some(lock) = read_guard.get(&id) {
                return lock.clone();
            }
        }
        // not found, write lock and create
        let mut write_guard = self.locks.write().unwrap();
        write_guard.entry(id).or_insert_with(|| Arc::new(RwLock::new(()))).clone()
    }

    /// Get single Being read lock object
    ///
    /// Return Arc<RwLock<()>>, caller manages locking
    pub fn read_lock(&self, id: BeingId) -> Result<BeingLock, DaoQLError> {
        let lock = self.get_or_create(id);
        // pre-check: try read lock, ensure lock available
        let _guard = lock.read().map_err(|_| {
            DaoQLError::Transaction(TransactionError::LockTimeout(id))
        })?;
        drop(_guard);
        Ok(lock)
    }

    /// Get single Being write lock object
    ///
    /// Return Arc<RwLock<()>>, caller manages locking
    pub fn write_lock(&self, id: BeingId) -> Result<BeingLock, DaoQLError> {
        let lock = self.get_or_create(id);
        // pre-check: try write lock, ensure lock available
        let _guard = lock.write().map_err(|_| {
            DaoQLError::Transaction(TransactionError::LockTimeout(id))
        })?;
        drop(_guard);
        Ok(lock)
    }

    /// BatchAcquire write lockobject（by BeingId sort，prevent deadlock）
    ///
    /// Educational Notes:
    /// - one necessary condition for deadlock is recurrent waiting
    /// - lock in global order, break recurrent waiting condition
    /// - this is a classic deadlock prevention policy in databases
    pub fn batch_write_lock(
        &self,
        mut ids: Vec<BeingId>,
    ) -> Result<Vec<BeingLock>, DaoQLError> {
        // Deduplicate and sort
        ids.sort();
        ids.dedup();

        let mut locks = Vec::with_capacity(ids.len());
        for id in ids {
            locks.push(self.write_lock(id)?);
        }
        Ok(locks)
    }

    /// Lock quantity (used for debugging)
    pub fn lock_count(&self) -> usize {
        self.locks.read().unwrap().len()
    }
}

impl LockManager {
    /// Get original lock object
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
                // simulate read operation
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
