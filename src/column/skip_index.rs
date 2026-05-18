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
//! Skip Index — Granule 级 min/max 元数据
//!
//! 教学说明：
//! - 每个 Granule（64KB 块）维护 min/max
//! - 查询时先检查 min/max，不匹配的 Granule 直接跳过
//! - 这是列式存储的核心优化之一
//! - 时间复杂度：O(Granule 数) 的剪枝，而非 O(记录数)


/// Granule 元数据
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GranuleMeta {
    /// 数据偏移
    pub offset: u64,
    /// 记录数
    pub count: u32,
    /// 最小值（8 bytes，足够存储 i64/f64）
    pub min_val: [u8; 8],
    /// 最大值
    pub max_val: [u8; 8],
}

impl GranuleMeta {
    pub fn new(offset: u64, count: u32) -> Self {
        Self {
            offset,
            count,
            min_val: [0; 8],
            max_val: [0; 8],
        }
    }

    /// 设置 min/max（f64）
    pub fn set_f64(&mut self, min: f64, max: f64) {
        self.min_val = min.to_le_bytes();
        self.max_val = max.to_le_bytes();
    }

    /// 获取 min（f64）
    pub fn min_f64(&self) -> f64 {
        f64::from_le_bytes(self.min_val)
    }

    /// 获取 max（f64）
    pub fn max_f64(&self) -> f64 {
        f64::from_le_bytes(self.max_val)
    }

    /// 判断值是否在范围内
    pub fn contains_f64(&self, value: f64) -> bool {
        value >= self.min_f64() && value <= self.max_f64()
    }

    /// 判断范围是否重叠
    pub fn overlaps_f64(&self, min: f64, max: f64) -> bool {
        !(self.max_f64() < min || self.min_f64() > max)
    }
}

/// Skip Index — Granule 元数据列表
pub struct SkipIndex {
    pub granules: Vec<GranuleMeta>,
}

impl SkipIndex {
    pub fn new() -> Self {
        Self { granules: Vec::new() }
    }

    /// 添加 Granule 元数据
    pub fn push(&mut self, meta: GranuleMeta) {
        self.granules.push(meta);
    }

    /// 查询：返回可能包含值的 Granule 索引列表
    pub fn query_f64(&self, min: f64, max: f64) -> Vec<usize> {
        self.granules
            .iter()
            .enumerate()
            .filter(|(_, g)| g.overlaps_f64(min, max))
            .map(|(i, _)| i)
            .collect()
    }

    /// 统计剪枝效果
    pub fn prune_stats(&self, min: f64, max: f64) -> (usize, usize) {
        let checked = self.query_f64(min, max).len();
        let total = self.granules.len();
        (checked, total)
    }
}

impl Default for SkipIndex {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skip_index_basic() {
        let mut idx = SkipIndex::new();
        let mut g1 = GranuleMeta::new(0, 100);
        g1.set_f64(0.0, 10.0);
        let mut g2 = GranuleMeta::new(1024, 100);
        g2.set_f64(20.0, 30.0);
        let mut g3 = GranuleMeta::new(2048, 100);
        g3.set_f64(5.0, 25.0);

        idx.push(g1);
        idx.push(g2);
        idx.push(g3);

        // 查询 [15, 22]，应匹配 g2 和 g3
        let result = idx.query_f64(15.0, 22.0);
        assert_eq!(result.len(), 2);
        assert!(result.contains(&1)); // g2
        assert!(result.contains(&2)); // g3
    }

    #[test]
    fn test_granule_contains() {
        let mut g = GranuleMeta::new(0, 10);
        g.set_f64(1.0, 10.0);
        assert!(g.contains_f64(5.0));
        assert!(!g.contains_f64(0.5));
        assert!(!g.contains_f64(10.5));
    }
}
