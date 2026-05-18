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
//! Version（版本管理）— 隐式 MVCC 机制
//!
//! 教学说明：
//! - DaoQL-Edu 采用隐式 MVCC：更新 = 追加新版本，旧版本保留
//! - 每个 NodeRecord 内联存储 tx_begin/tx_end + prev/next_version_offset
//! - 无需额外版本表，版本链指针直接内联在图节点中
//! - 查询支持：history: all / last(N) / at(ts) / as_of(ts)
//!
//! MVCC 核心规则（Read Committed）：
//! - 事务只能看到已提交的版本
//! - 更新不覆盖旧数据，而是创建新版本并设置旧版本的 tx_end
//! - 事务 ID 单调递增，用于判断版本的时间顺序

use crate::being::BeingCore;
use crate::error::DaoQLError;
use crate::id::BeingId;
use crate::transaction::TxId;

/// 版本链节点（内存表示）
///
/// 对应 mmap 中的 NodeRecord，提取出版本相关字段。
#[derive(Debug, Clone, PartialEq)]
pub struct VersionNode {
    /// 实体 ID
    pub being_id: BeingId,
    /// 版本开始事务号
    pub tx_begin: TxId,
    /// 版本结束事务号（u64::MAX = 当前活跃版本）
    pub tx_end: TxId,
    /// 上一个版本偏移（mmap 中的 NodeOffset）
    pub prev_version_offset: u64,
    /// 下一个版本偏移
    pub next_version_offset: u64,
    /// 创建时间
    pub created_at: i64,
    /// 版本内容（BeingCore）
    pub core: BeingCore,
}

/// 版本链
///
/// 一个 Being 的所有版本按时间顺序组成的链表。
/// 头节点是最新版本，尾节点是最早版本。
pub struct VersionChain {
    /// BeingId
    pub being_id: BeingId,
    /// 最新版本偏移（head）
    pub head_offset: u64,
    /// 最早版本偏移（tail）
    pub tail_offset: u64,
    /// 版本数量
    pub count: usize,
}

impl VersionChain {
    /// 创建空版本链
    pub fn new(being_id: BeingId) -> Self {
        Self {
            being_id,
            head_offset: 0,
            tail_offset: 0,
            count: 0,
        }
    }

    /// 是否为空
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }
}

/// 版本查询模式
#[derive(Debug, Clone, Copy, PartialEq)]
#[derive(Default)]
pub enum HistoryMode {
    /// 当前版本（默认）
    #[default]
    Current,
    /// 所有历史版本
    All,
    /// 最近 N 个版本
    Last(usize),
    /// 指定事务号时的版本
    At(TxId),
    /// 指定时间戳时的版本
    AsOf(i64),
}


/// MVCC 可见性判断
///
/// 判断一个版本（tx_begin/tx_end）在当前事务下是否可见。
///
/// 参数：
/// - version_begin: 版本的创建事务号
/// - version_end: 版本的结束事务号（u64::MAX = 未结束）
/// - reader_tx: 读取者的事务号
/// - active_txs: 当前活跃事务集合
///
/// 规则：
/// 1. 自己创建的版本总是可见
/// 2. 创建者已提交（不在 active_txs 中）
/// 3. 版本未结束，或结束者已提交
pub fn is_visible(
    version_begin: TxId,
    version_end: TxId,
    reader_tx: TxId,
    active_txs: &[TxId],
) -> bool {
    // 规则 1：自己创建的版本总是可见
    if version_begin == reader_tx {
        return true;
    }

    // 规则 2：创建者已提交
    let creator_committed = !active_txs.contains(&version_begin);
    if !creator_committed {
        return false;
    }

    // 规则 3：版本未结束，或结束者已提交
    let is_ended = version_end != u64::MAX;
    if !is_ended {
        return true; // 未结束 = 当前活跃版本
    }

    
    !active_txs.contains(&version_end)
}

/// 版本管理器（内存索引）
///
/// 维护 BeingId → VersionChain 的映射，用于快速定位版本链。
/// 教学版简化：内存 HashMap，重启后从 mmap 重建。
pub struct VersionManager {
    chains: std::collections::HashMap<BeingId, VersionChain>,
}

impl VersionManager {
    pub fn new() -> Self {
        Self {
            chains: std::collections::HashMap::new(),
        }
    }

    /// 注册新版本
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

    /// 获取版本链
    pub fn get_chain(&self, being_id: BeingId) -> Option<&VersionChain> {
        self.chains.get(&being_id)
    }

    /// 获取版本数量
    pub fn version_count(&self, being_id: BeingId) -> usize {
        self.chains.get(&being_id).map(|c| c.count).unwrap_or(0)
    }

    /// 重建版本链（启动时从 mmap 扫描）
    ///
    /// 教学说明：
    /// - 启动时遍历所有 NodeRecord，按 BeingId 分组构建链表
    /// - 时间复杂度 O(N)，N = 节点数
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
        // 版本由 tx=10 创建，未结束
        // reader tx=20，活跃事务集为空
        assert!(is_visible(10, u64::MAX, 20, &[]));

        // 版本由 tx=10 创建，reader 自己就是创建者
        assert!(is_visible(10, u64::MAX, 10, &[]));

        // 版本由 tx=10 创建，但 tx=10 仍在活跃中（未提交）
        assert!(!is_visible(10, u64::MAX, 20, &[10]));
    }

    #[test]
    fn test_mvcc_visibility_ended() {
        // 版本由 tx=10 创建，tx=15 结束，两者都已提交
        assert!(is_visible(10, 15, 20, &[]));

        // 版本由 tx=10 创建，tx=15 结束，但 tx=15 仍在活跃
        assert!(!is_visible(10, 15, 20, &[15]));

        // 版本已结束，reader 在结束后才启动
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
