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

use crate::being::BeingCore;
use crate::error::DaoQLError;
use crate::id::BeingId;
use crate::transaction::TxId;

/// Version chain node (in-memory representation)
///
/// Corresponds to NodeRecord in mmap, extracting version-related fields.
#[derive(Debug, Clone, PartialEq)]
pub struct VersionNode {
    /// Entity ID
    pub being_id: BeingId,
    /// Version begin transaction number
    pub tx_begin: TxId,
    /// Version end transaction number (u64::MAX = current active version)
    pub tx_end: TxId,
    /// Previous version offset (NodeOffset in mmap)
    pub prev_version_offset: u64,
    /// Next version offset
    pub next_version_offset: u64,
    /// Creation time
    pub created_at: i64,
    /// Version content (BeingCore)
    pub core: BeingCore,
}

/// Version chain
///
/// A linked list of all versions of a Being in chronological order.
/// Head node is the latest version, tail node is the earliest version.
pub struct VersionChain {
    /// BeingId
    pub being_id: BeingId,
    /// Latest version offset (head)
    pub head_offset: u64,
    /// Earliest version offset (tail)
    pub tail_offset: u64,
    /// Version count
    pub count: usize,
}

impl VersionChain {
    /// Create empty version chain
    pub fn new(being_id: BeingId) -> Self {
        Self {
            being_id,
            head_offset: 0,
            tail_offset: 0,
            count: 0,
        }
    }

    /// Is empty
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }
}

/// Version query mode
#[derive(Debug, Clone, Copy, PartialEq)]
#[derive(Default)]
pub enum HistoryMode {
    /// Current version (default)
    #[default]
    Current,
    /// All historical versions
    All,
    /// Last N versions
    Last(usize),
    /// Version at specified transaction number
    At(TxId),
    /// Version at specified timestamp
    AsOf(i64),
}


/// MVCC visibility check
///
/// Check whether a version (tx_begin/tx_end) is visible under current transaction.
///
/// Parameter：
/// - version_begin: version's create transaction number
/// - version_end: version's end transaction number (u64::MAX = not ended)
/// - reader_tx: reader's transaction number
/// - active_txs: current active transaction set
///
/// Rule：
/// 1. Self-created version is always visible
/// 2. Creator committed (not in active_txs)
/// 3. Version not ended, or ender committed
pub fn is_visible(
    version_begin: TxId,
    version_end: TxId,
    reader_tx: TxId,
    active_txs: &[TxId],
) -> bool {
    // Rule 1: Self-created version is always visible
    if version_begin == reader_tx {
        return true;
    }

    // Rule 2: Creator committed
    let creator_committed = !active_txs.contains(&version_begin);
    if !creator_committed {
        return false;
    }

    // Rule 3: Version not ended, or ender committed
    let is_ended = version_end != u64::MAX;
    if !is_ended {
        return true; // Not ended = current active version
    }

    
    !active_txs.contains(&version_end)
}

/// Version manager (in-memory index)
///
/// Maintains BeingId → VersionChain mapping for quick version chain lookup.
/// edu edition simplification: in-memory HashMap, rebuilt from mmap on restart.
pub struct VersionManager {
    chains: std::collections::HashMap<BeingId, VersionChain>,
}

impl VersionManager {
    pub fn new() -> Self {
        Self {
            chains: std::collections::HashMap::new(),
        }
    }

    /// Register new version
    pub fn register_version(
        &mut self,
        being_id: BeingId,
        offset: u64,
    ) -> Result<(), DaoQLError> {
        let chain = self.chains.entry(being_id).or_insert_with(|| {
            VersionChain::new(being_id)
        });

        if chain.is_empty() {
            chain.head_offset = offset;
            chain.tail_offset = offset;
        } else {
            chain.head_offset = offset;
        }
        chain.count += 1;
        Ok(())
    }

    /// Get version chain
    pub fn get_chain(&self, being_id: BeingId) -> Option<&VersionChain> {
        self.chains.get(&being_id)
    }

    /// Get version count
    pub fn version_count(&self, being_id: BeingId) -> usize {
        self.chains.get(&being_id).map(|c| c.count).unwrap_or(0)
    }

    /// Rebuild version chain (scan from mmap at startup)
    ///
    /// Educational Notes:
    /// - At startup, iterate all NodeRecords, build linked list grouped by BeingId
    /// - Time complexity O(N), N = node count
    pub fn rebuild(&mut self, graph: &crate::graph::store::GraphStore) -> Result<(), DaoQLError> {
        self.chains.clear();
        for i in 0..graph.node_count() {
            let offset = (i * crate::graph::record::NodeRecord::SIZE) as u64;
            if let Ok(node) = graph.read_node(offset) {
                if node.is_empty() {
                    continue;
                }
                let chain = self.chains.entry(node.id).or_insert_with(|| {
                    VersionChain::new(node.id)
                });
                if chain.is_empty() {
                    chain.head_offset = offset;
                    chain.tail_offset = offset;
                } else {
                    chain.head_offset = offset;
                }
                chain.count += 1;
            }
        }
        Ok(())
    }
}

impl Default for VersionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mvcc_visibility_basic() {
        // Version created by tx=10, not ended
        // reader tx=20, active transaction set is empty
        assert!(is_visible(10, u64::MAX, 20, &[]));

        // Version created by tx=10, reader is the creator
        assert!(is_visible(10, u64::MAX, 10, &[]));

        // Version created by tx=10, but tx=10 is still active (not committed)
        assert!(!is_visible(10, u64::MAX, 20, &[10]));
    }

    #[test]
    fn test_mvcc_visibility_ended() {
        // Version created by tx=10, ended by tx=15, both committed
        assert!(is_visible(10, 15, 20, &[]));

        // Version created by tx=10, ended by tx=15, but tx=15 is still active
        assert!(!is_visible(10, 15, 20, &[15]));

        // Version ended, reader started after end
        assert!(is_visible(10, 15, 20, &[]));
    }

    #[test]
    fn test_version_chain() {
        let id = BeingId::new();
        let mut vm = VersionManager::new();

        vm.register_version(id, 100).unwrap();
        assert_eq!(vm.version_count(id), 1);

        vm.register_version(id, 200).unwrap();
        assert_eq!(vm.version_count(id), 2);

        let chain = vm.get_chain(id).unwrap();
        assert_eq!(chain.head_offset, 200);
        assert_eq!(chain.tail_offset, 100);
        assert_eq!(chain.count, 2);
    }

    #[test]
    fn test_history_mode() {
        assert_eq!(HistoryMode::default(), HistoryMode::Current);
        assert_eq!(HistoryMode::Last(5), HistoryMode::Last(5));
        assert_ne!(HistoryMode::All, HistoryMode::Current);
    }
}
