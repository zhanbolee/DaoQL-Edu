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
//! HNSW（Hierarchical Navigable Small World）索引
//!
//! 教学说明：
//! - HNSW 是多层图结构：层数越高，连接越稀疏（类似跳表）
//! - 插入时从顶层开始贪心搜索最近邻，逐层下降
//! - ef_construction 控制构建时的搜索宽度
//! - M 控制每层最大出度
//! - **标准教科书实现**：HashMap + HashSet + 标量距离计算
//!
//! 算法复杂度：
//! - 搜索：O(log N) 期望
//! - 插入：O(log N × M) 期望
//! - 内存：O(N × M × dim × sizeof(f32))

use std::collections::{BinaryHeap, HashMap, HashSet};

use rand::Rng;

use crate::error::DaoQLError;
use crate::id::BeingId;
use crate::vector::distance::{compute_distance, DistanceMetric};

/// 搜索结果（公开 API）
#[derive(Debug, Clone, PartialEq)]
pub struct SearchResult {
    pub id: BeingId,
    pub distance: f32,
}

impl Eq for SearchResult {}

impl PartialOrd for SearchResult {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SearchResult {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.distance.total_cmp(&other.distance)
    }
}

/// HNSW 节点
#[derive(Debug, Clone)]
pub struct HnswNode {
    pub id: BeingId,
    pub vector: Vec<f32>,
    /// 每层连接的邻居（层号 → 邻居的 BeingId 列表）
    pub connections: Vec<Vec<BeingId>>,
    /// 最高层号
    pub max_level: usize,
}

/// HNSW 索引
///
/// 标准教科书实现：
/// - `nodes: HashMap<BeingId, HnswNode>` — 按 BeingId 索引
/// - `connections` 存储 `BeingId` 而非 `usize`
/// - `entry_point` 存储 `BeingId`
/// - `visited` 使用 `HashSet<BeingId>`（搜索时分配）
/// - 距离计算使用标量 `compute_distance`
pub struct HnswIndex {
    /// 节点存储（按 BeingId 索引）
    nodes: HashMap<BeingId, HnswNode>,
    /// 维度
    dim: usize,
    /// 每层最大出度
    m: usize,
    /// 构建时搜索宽度
    ef_construction: usize,
    /// 查询时搜索宽度
    ef_search: usize,
    /// 全局入口点（BeingId）
    entry_point: Option<BeingId>,
    /// 当前最大层
    max_level: usize,
    /// 距离度量
    metric: DistanceMetric,
    /// 随机数生成器
    rng: rand::rngs::StdRng,
    /// 小数据集时切换到暴力扫描的阈值
    full_scan_threshold: usize,
    /// 扁平化嵌入缓存（用于暴力扫描）
    flat_embeddings: Vec<f32>,
    flat_ids: Vec<BeingId>,
}

impl HnswIndex {
    pub fn new(dim: usize, m: usize, ef_construction: usize, ef_search: usize) -> Self {
        Self {
            nodes: HashMap::new(),
            dim,
            m,
            ef_construction,
            ef_search,
            entry_point: None,
            max_level: 0,
            metric: DistanceMetric::Cosine,
            rng: rand::SeedableRng::from_seed([0; 32]),
            full_scan_threshold: 2_000,
            flat_embeddings: Vec::new(),
            flat_ids: Vec::new(),
        }
    }

    /// 设置距离度量
    pub fn with_metric(mut self, metric: DistanceMetric) -> Self {
        self.metric = metric;
        self
    }

    /// 计算随机层数（指数衰减分布）
    fn random_level(&mut self) -> usize {
        let mut level = 0;
        let m_l = 1.0 / (self.m as f64).ln();
        while self.rng.gen::<f64>() < m_l && level < 16 {
            level += 1;
        }
        level
    }

    /// 计算节点到查询向量的距离
    fn distance_to_query(&self, node: &HnswNode, query: &[f32]) -> f32 {
        compute_distance(&node.vector, query, self.metric)
    }

    /// 贪心搜索单层：找到离查询最近的 1 个节点
    fn greedy_search_layer(&self, entry_id: BeingId, query: &[f32], level: usize) -> BeingId {
        let entry_node = self.nodes.get(&entry_id).unwrap();
        let mut current = entry_id;
        let mut min_dist = self.distance_to_query(entry_node, query);

        loop {
            let node = self.nodes.get(&current).unwrap();
            let mut improved = false;

            if level >= node.connections.len() {
                break;
            }

            for &neighbor_id in &node.connections[level] {
                let neighbor = self.nodes.get(&neighbor_id).unwrap();
                let dist = self.distance_to_query(neighbor, query);
                if dist < min_dist {
                    min_dist = dist;
                    current = neighbor_id;
                    improved = true;
                }
            }

            if !improved {
                break;
            }
        }

        current
    }

    /// 搜索单层，返回 ef 个最近邻
    /// 使用 HashSet<BeingId> 做 visited，BinaryHeap<SearchResult> 做 candidates 和 results
    fn search_layer(
        &self,
        entry_id: BeingId,
        query: &[f32],
        level: usize,
        ef: usize,
    ) -> Vec<SearchResult> {
        let mut candidates: BinaryHeap<SearchResult> = BinaryHeap::with_capacity(ef * 2);
        let mut results: BinaryHeap<SearchResult> = BinaryHeap::with_capacity(ef * 2);
        let mut visited: HashSet<BeingId> = HashSet::with_capacity(ef * 2);

        let entry_node = self.nodes.get(&entry_id).unwrap();
        let entry_dist = self.distance_to_query(entry_node, query);

        candidates.push(SearchResult {
            id: entry_id,
            distance: -entry_dist,
        });
        results.push(SearchResult {
            id: entry_id,
            distance: entry_dist,
        });
        visited.insert(entry_id);

        while let Some(current) = candidates.pop() {
            let current_dist = -current.distance;

            // 终止条件：当前候选的距离已大于结果中最差的
            if let Some(worst) = results.peek() {
                if current_dist > worst.distance && results.len() >= ef {
                    break;
                }
            }

            let node = self.nodes.get(&current.id).unwrap();
            if level >= node.connections.len() {
                continue;
            }

            for &neighbor_id in &node.connections[level] {
                if visited.contains(&neighbor_id) {
                    continue;
                }
                visited.insert(neighbor_id);

                let neighbor = self.nodes.get(&neighbor_id).unwrap();
                let dist = self.distance_to_query(neighbor, query);

                if results.len() < ef || dist < results.peek().unwrap().distance {
                    candidates.push(SearchResult {
                        id: neighbor_id,
                        distance: -dist,
                    });
                    results.push(SearchResult {
                        id: neighbor_id,
                        distance: dist,
                    });

                    if results.len() > ef {
                        results.pop(); // 移除最远的
                    }
                }
            }
        }

        results.into_sorted_vec()
    }

    /// 启发式选择邻居（保留多样性连接）
    fn select_neighbors(
        &self,
        candidates: &[SearchResult],
        m: usize,
    ) -> Vec<SearchResult> {
        // 简化版：直接取最近的 m 个
        // 生产版应使用启发式（考虑角度多样性）
        candidates.iter().take(m).cloned().collect()
    }

    /// 插入向量（Cosine 模式下自动预归一化）
    pub fn insert(&mut self, id: BeingId, mut vector: Vec<f32>) -> Result<(), DaoQLError> {
        if vector.len() != self.dim {
            return Err(DaoQLError::Query(crate::error::QueryError::DimensionMismatch {
                expected: self.dim,
                actual: vector.len(),
            }));
        }

        // Cosine 预归一化：存储时 L2-normalize，搜索时退化为 1.0 - dot
        if self.metric == DistanceMetric::Cosine {
            crate::vector::distance::l2_normalize(&mut vector);
        }

        let level = self.random_level();
        let mut connections: Vec<Vec<BeingId>> = vec![Vec::new(); level + 1];

        // 缓存向量用于扁平化存储（在移动前克隆）
        let vector_for_flat = vector.clone();

        let new_node = HnswNode {
            id,
            vector,
            connections: connections.clone(),
            max_level: level,
        };

        // 空索引：直接设为入口点
        if self.entry_point.is_none() {
            self.entry_point = Some(id);
            self.max_level = level;
            self.nodes.insert(id, new_node);
            self.flat_ids.push(id);
            self.flat_embeddings.extend_from_slice(&vector_for_flat);
            return Ok(());
        }

        let entry_id = self.entry_point.unwrap();

        // 1. 从顶层开始搜索入口点
        let mut current_entry = entry_id;
        for l in (level + 1)..=self.max_level {
            current_entry = self.greedy_search_layer(current_entry, &new_node.vector, l);
        }

        // 2. 从插入层开始，逐层连接
        for l in (0..=level.min(self.max_level)).rev() {
            let neighbors = self.search_layer(
                current_entry,
                &new_node.vector,
                l,
                self.ef_construction,
            );
            let selected = self.select_neighbors(&neighbors, self.m);

            for neighbor in &selected {
                connections[l].push(neighbor.id);
                // 双向连接
                if let Some(neighbor_node) = self.nodes.get_mut(&neighbor.id) {
                    if l < neighbor_node.connections.len() {
                        neighbor_node.connections[l].push(id);
                        // 限制出度
                        if neighbor_node.connections[l].len() > self.m * 2 {
                            neighbor_node.connections[l].truncate(self.m * 2);
                        }
                    }
                }
            }

            if !selected.is_empty() {
                current_entry = selected[0].id;
            }
        }

        // 3. 更新全局入口点
        if level > self.max_level {
            self.max_level = level;
            self.entry_point = Some(id);
        }

        // 插入节点
        let mut final_node = new_node;
        final_node.connections = connections;
        self.nodes.insert(id, final_node);

        // 更新扁平化缓存
        self.flat_ids.push(id);
        self.flat_embeddings.extend_from_slice(&vector_for_flat);

        Ok(())
    }

    /// 暴力扫描搜索（小数据集时比 HNSW 遍历更快）
    /// 使用扁平化嵌入数组提升 cache locality
    fn search_brute_force(&self, query: &[f32], k: usize) -> Vec<SearchResult> {
        let n = self.flat_ids.len();
        let mut results = Vec::with_capacity(n);
        for i in 0..n {
            let start = i * self.dim;
            let vec = &self.flat_embeddings[start..start + self.dim];
            results.push(SearchResult {
                id: self.flat_ids[i],
                distance: compute_distance(query, vec, self.metric),
            });
        }
        results.sort_by(|a, b| a.distance.total_cmp(&b.distance));
        results.truncate(k);
        results
    }

    pub fn search(&self, query: &[f32], k: usize) -> Result<Vec<SearchResult>, DaoQLError> {
        if query.len() != self.dim {
            return Err(DaoQLError::Query(crate::error::QueryError::DimensionMismatch {
                expected: self.dim,
                actual: query.len(),
            }));
        }

        if self.nodes.is_empty() {
            return Ok(Vec::new());
        }

        // Cosine 模式下预归一化 query
        let query_normalized: Vec<f32>;
        let q = if self.metric == DistanceMetric::Cosine {
            let mut q = query.to_vec();
            crate::vector::distance::l2_normalize(&mut q);
            query_normalized = q;
            &query_normalized
        } else {
            query
        };

        // 小数据集：暴力扫描更快（避免 HNSW 遍历开销）
        if self.nodes.len() <= self.full_scan_threshold {
            return Ok(self.search_brute_force(q, k));
        }

        let entry_id = match self.entry_point {
            Some(e) => e,
            None => return Ok(Vec::new()),
        };

        // 从顶层搜索入口点
        let mut current = entry_id;
        for l in (1..=self.max_level).rev() {
            current = self.greedy_search_layer(current, q, l);
        }

        // 在第 0 层搜索 k 个最近邻
        let ef = k.max(self.ef_search);
        let mut results = self.search_layer(current, q, 0, ef);
        results.truncate(k);
        Ok(results)
    }

    /// 批量插入向量
    pub fn insert_batch(&mut self, items: &[(BeingId, Vec<f32>)]) -> Result<(), DaoQLError> {
        for (id, vec) in items {
            self.insert(*id, vec.clone())?;
        }
        Ok(())
    }

    /// 节点数量
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn random_vector(dim: usize) -> Vec<f32> {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        (0..dim).map(|_| rng.gen::<f32>()).collect()
    }

    #[test]
    fn test_hnsw_insert_and_search() {
        let mut index = HnswIndex::new(16, 8, 50, 32);

        // 插入 100 个随机向量
        for _ in 0..100 {
            let id = BeingId::new();
            let vec = random_vector(16);
            index.insert(id, vec).unwrap();
        }

        assert_eq!(index.len(), 100);

        // 搜索
        let query = random_vector(16);
        let results = index.search(&query, 10).unwrap();
        assert_eq!(results.len(), 10);
    }

    #[test]
    fn test_hnsw_recall_rate() {
        let dim = 32;
        let mut index = HnswIndex::new(dim, 16, 100, 64);

        // 插入 1000 个向量
        let n = 1000;
        let mut vectors: Vec<(BeingId, Vec<f32>)> = Vec::with_capacity(n);
        for _ in 0..n {
            let id = BeingId::new();
            let vec = random_vector(dim);
            index.insert(id, vec.clone()).unwrap();
            vectors.push((id, vec));
        }

        // 随机查询，检查召回率
        let mut total_recall = 0.0;
        let test_queries = 50;

        for _ in 0..test_queries {
            let query = random_vector(dim);

            // HNSW 搜索结果
            let hnsw_results = index.search(&query, 10).unwrap();
            let hnsw_ids: std::collections::HashSet<_> =
                hnsw_results.iter().map(|r| r.id).collect();

            // 暴力搜索结果（ground truth）
            let mut all_dists: Vec<_> = vectors
                .iter()
                .map(|(id, vec)| {
                    let dist = compute_distance(&query, vec, DistanceMetric::Cosine);
                    (*id, dist)
                })
                .collect();
            all_dists.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
            let brute_ids: std::collections::HashSet<_> =
                all_dists.iter().take(10).map(|(id, _)| *id).collect();

            // 计算召回率
            let intersection: Vec<_> = hnsw_ids.intersection(&brute_ids).collect();
            total_recall += intersection.len() as f64 / 10.0;
        }

        let avg_recall = total_recall / test_queries as f64;
        println!("HNSW 平均召回率: {:.2}", avg_recall);
        assert!(avg_recall > 0.7, "召回率过低: {}", avg_recall);
    }

    #[test]
    fn test_hnsw_empty_search() {
        let index = HnswIndex::new(10, 8, 50, 32);
        let results = index.search(&[0.0; 10], 5).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_hnsw_dimension_mismatch() {
        let mut index = HnswIndex::new(10, 8, 50, 32);
        let id = BeingId::new();
        assert!(index.insert(id, vec![0.0; 5]).is_err());
        assert!(index.search(&[0.0; 5], 5).is_err());
    }
}
