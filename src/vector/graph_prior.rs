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
//! 图先验插入 — 利用 Relation 邻接节点作为 HNSW 搜索种子
//!
//! 教学说明：
//! - 传统 HNSW 插入：从全局入口点开始搜索
//! - 图先验：如果新节点与已知节点有 Relation，从邻接节点开始搜索
//! - 效果：有关系的节点在向量空间中通常也相近，减少搜索步数
//! - 特别适用于：知识图谱 + 向量联合场景

use crate::error::DaoQLError;
use crate::id::BeingId;
use crate::vector::hnsw::HnswIndex;

/// 图先验种子选择器
pub struct GraphPrior;

impl GraphPrior {
    /// 选择 HNSW 插入的种子节点
    ///
    /// 策略：
    /// 1. 如果邻接节点已存在于 HNSW 中，返回这些节点作为种子
    /// 2. 否则，返回全局入口点
    pub fn select_seed(
        _hnsw: &HnswIndex,
        _being_id: BeingId,
        _neighbors: &[BeingId],
    ) -> Result<Vec<BeingId>, DaoQLError> {
        // 教学版简化：直接返回邻接节点
        // 生产版应检查节点是否已在 HNSW 中
        Ok(_neighbors.to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_graph_prior_seed() {
        let hnsw = HnswIndex::new(10, 8, 50, 32);
        let neighbors = vec![BeingId::new(), BeingId::new()];
        let seeds = GraphPrior::select_seed(&hnsw, BeingId::new(), &neighbors).unwrap();
        assert_eq!(seeds.len(), 2);
    }
}
