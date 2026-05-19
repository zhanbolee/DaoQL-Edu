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

pub mod committer;
pub mod lock_manager;
pub mod validator;

pub use committer::Transaction;
pub use lock_manager::LockManager;
pub use validator::ConstraintValidator;

/// Transaction ID (monotonically increasing)
pub type TxId = u64;

/// Active transaction set
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
