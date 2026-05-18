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
//! 图遍历算法
//!
//! 教学说明：
//! - BFS（广度优先搜索）：层序遍历，适合最短路径
//! - DFS（深度优先搜索）：递归/栈遍历，适合连通分量
//! - PageRank：经典图算法，衡量节点重要性
//!
//! 所有算法基于免索引邻接链表，时间复杂度：
//! - BFS/DFS: O(V + E)
//! - PageRank: O(iterations × E)

use std::collections::{HashMap, HashSet, VecDeque};

use crate::error::DaoQLError;
use crate::graph::record::{EdgeRecord, NodeRecord};
use crate::graph::store::GraphStore;
use crate::id::{BeingId, NodeOffset};

/// 边过滤器
#[derive(Clone)]
pub struct EdgeFilter {
    /// 限定关系类型（None = 全部）
    pub relation_type: Option<u16>,
    /// 限定有向/无向（None = 全部）
    pub directed: Option<bool>,
    /// 最大权重
    pub max_weight: Option<f64>,
}

impl EdgeFilter {
    /// 接受所有边
    pub fn all() -> Self {
        Self {
            relation_type: None,
            directed: None,
            max_weight: None,
        }
    }

    /// 按关系类型过滤
    pub fn by_type(code: u16) -> Self {
        Self {
            relation_type: Some(code),
            directed: None,
            max_weight: None,
        }
    }

    /// 判断是否匹配
    pub fn matches(&self, edge: &EdgeRecord) -> bool {
        if let Some(rt) = self.relation_type {
            if edge.relation_type != rt {
                return false;
            }
        }
        if let Some(d) = self.directed {
            let edge_directed = edge.directed != 0;
            if edge_directed != d {
                return false;
            }
        }
        if let Some(max_w) = self.max_weight {
            if edge.weight > max_w {
                return false;
            }
        }
        true
    }
}

/// BFS 遍历
///
/// 参数：
/// - start: 起始节点偏移
/// - depth: 最大深度（0 = 仅起始节点）
/// - filter: 边过滤器
///
/// 返回：按遍历顺序的 (节点偏移, 深度) 列表
pub fn bfs(
    store: &mut GraphStore,
    start: NodeOffset,
    depth: usize,
    filter: Option<&EdgeFilter>,
) -> Result<Vec<(NodeOffset, usize)>, DaoQLError> {
    let mut visited = HashSet::new();
    let mut result = Vec::new();
    let mut queue = VecDeque::new();

    queue.push_back((start, 0));
    visited.insert(start);
    result.push((start, 0));

    while let Some((current_offset, current_depth)) = queue.pop_front() {
        if current_depth >= depth {
            continue;
        }

        let to_ids: Vec<BeingId> = store.out_edges(current_offset)?
            .iter()
            .filter(|edge| filter.map_or(true, |f| f.matches(edge)))
            .map(|edge| edge.to_id)
            .collect();
        for to_id in to_ids {
            // 找到目标节点偏移
            let target_offset = store.find_node_offset(to_id)?;
            if !visited.contains(&target_offset) {
                visited.insert(target_offset);
                result.push((target_offset, current_depth + 1));
                queue.push_back((target_offset, current_depth + 1));
            }
        }
    }

    Ok(result)
}

/// DFS 遍历
///
/// 参数：
/// - start: 起始节点偏移
/// - depth: 最大深度
/// - filter: 边过滤器
///
/// 返回：按遍历顺序的 (节点偏移, 深度) 列表
pub fn dfs(
    store: &mut GraphStore,
    start: NodeOffset,
    depth: usize,
    filter: Option<&EdgeFilter>,
) -> Result<Vec<(NodeOffset, usize)>, DaoQLError> {
    let mut visited = HashSet::new();
    let mut result = Vec::new();

    dfs_recursive(store, start, 0, depth, filter, &mut visited, &mut result)?;

    Ok(result)
}

fn dfs_recursive(
    store: &mut GraphStore,
    offset: NodeOffset,
    current_depth: usize,
    max_depth: usize,
    filter: Option<&EdgeFilter>,
    visited: &mut HashSet<NodeOffset>,
    result: &mut Vec<(NodeOffset, usize)>,
) -> Result<(), DaoQLError> {
    if current_depth > max_depth {
        return Ok(());
    }

    visited.insert(offset);
    result.push((offset, current_depth));

    if current_depth < max_depth {
        let to_ids: Vec<BeingId> = store.out_edges(offset)?
            .iter()
            .filter(|edge| filter.map_or(true, |f| f.matches(edge)))
            .map(|edge| edge.to_id)
            .collect();
        for to_id in to_ids {
            let target_offset = store.find_node_offset(to_id)?;
            if !visited.contains(&target_offset) {
                dfs_recursive(
                    store,
                    target_offset,
                    current_depth + 1,
                    max_depth,
                    filter,
                    visited,
                    result,
                )?;
            }
        }
    }

    Ok(())
}

/// PageRank 算法
///
/// 教学说明：
/// - PageRank 是 Google 搜索引擎的核心算法
/// - 思想：一个节点的重要性 = 所有指向它的节点的重要性之和（加权）
/// - 公式：PR(u) = (1-d)/N + d × Σ(PR(v)/L(v))
///   - d: 阻尼系数（通常 0.85）
///   - N: 总节点数
///   - L(v): v 的出边数
///   - Σ: 对所有指向 u 的节点 v 求和
///
/// 参数：
/// - iterations: 迭代次数
/// - damping: 阻尼系数
///
/// 返回：BeingId → PageRank 值
pub fn pagerank(
    store: &mut GraphStore,
    iterations: usize,
    damping: f64,
) -> Result<HashMap<BeingId, f64>, DaoQLError> {
    let n = store.node_count();
    if n == 0 {
        return Ok(HashMap::new());
    }

    let n_f64 = n as f64;
    let base = (1.0 - damping) / n_f64;

    // 初始化：所有节点 PR = 1/N
    let mut pr: HashMap<BeingId, f64> = HashMap::new();
    for i in 0..n {
        let offset = (i * NodeRecord::SIZE) as u64;
        let node = store.read_node(offset)?;
        pr.insert(node.id, 1.0 / n_f64);
    }

    // 迭代
    for _ in 0..iterations {
        let mut new_pr: HashMap<BeingId, f64> = HashMap::new();

        // 计算所有 sink 节点（无出边）的 PR 总和，重新分配给所有节点
        let mut sink_pr = 0.0;
        for i in 0..n {
            let offset = (i * NodeRecord::SIZE) as u64;
            if store.out_edges(offset)?.is_empty() {
                let node_id = store.read_node(offset)?.id;
                sink_pr += pr.get(&node_id).unwrap_or(&0.0);
            }
        }
        let sink_share = damping * sink_pr / n_f64;

        for i in 0..n {
            let offset = (i * NodeRecord::SIZE) as u64;
            let node_id = store.read_node(offset)?.id;
            let mut rank = base + sink_share;

            // 获取所有入边
            let in_edge_ids: Vec<BeingId> = store.in_edges(offset)?
                .iter()
                .map(|edge| edge.from_id)
                .collect();
            for from_id in in_edge_ids {
                let from_offset = store.find_node_offset(from_id)?;
                let from_out_count = store.out_edges(from_offset)?.len().max(1) as f64;
                let from_pr = *pr.get(&from_id).unwrap_or(&0.0);
                rank += damping * from_pr / from_out_count;
            }

            new_pr.insert(node_id, rank);
        }

        pr = new_pr;
    }

    Ok(pr)
}

/// 查找最短路径（BFS）
///
/// 返回路径上的节点偏移列表（包含起点和终点）
pub fn shortest_path(
    store: &mut GraphStore,
    start: NodeOffset,
    target: BeingId,
    filter: Option<&EdgeFilter>,
) -> Result<Vec<NodeOffset>, DaoQLError> {
    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();
    let mut parent: HashMap<NodeOffset, NodeOffset> = HashMap::new();

    queue.push_back(start);
    visited.insert(start);

    while let Some(current) = queue.pop_front() {
        let node_id = store.read_node(current)?.id;
        if node_id == target {
            // 重建路径
            let mut path = vec![current];
            let mut cur = current;
            while let Some(&p) = parent.get(&cur) {
                path.push(p);
                cur = p;
            }
            path.reverse();
            return Ok(path);
        }

        let to_ids: Vec<BeingId> = store.out_edges(current)?
            .iter()
            .filter(|edge| filter.map_or(true, |f| f.matches(edge)))
            .map(|edge| edge.to_id)
            .collect();
        for to_id in to_ids {
            let target_offset = store.find_node_offset(to_id)?;
            if !visited.contains(&target_offset) {
                visited.insert(target_offset);
                parent.insert(target_offset, current);
                queue.push_back(target_offset);
            }
        }
    }

    Err(DaoQLError::InvalidState("路径不存在".to_string()))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::being::BeingCore;
    use crate::config::Config;
    use crate::relation::Relation;
    use crate::storage::StorageManager;

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join("daoql-edu-test-traverse").join(name);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn build_test_graph(name: &str) -> (GraphStore, Vec<BeingId>) {
        let dir = temp_dir(name);
        let storage = Arc::new(StorageManager::open(&dir, Config::default()).unwrap());
        let mut store = GraphStore::new(storage);
        store.set_tx(1);

        // 创建 5 个节点：A -> B -> C -> D, A -> E
        let ids: Vec<_> = (0..5).map(|_| BeingId::new()).collect();

        for (i, &id) in ids.iter().enumerate() {
            let core = BeingCore {
                id,
                def: "Test".to_string(),
                status: 0,
                name: format!("Node{}", i),
                code: "".to_string(),
                description: "".to_string(),
                weight: 0.0,
                priority: 0,
                category: "".to_string(),
                created_at: 0,
                updated_at: 0,
                tx_begin: 1,
                tx_end: u64::MAX,
            };
            store.create_node(&core, 0).unwrap();
        }

        // A(0) -> B(1)
        store.create_edge(&Relation::new(ids[0], ids[1], 1, true)).unwrap();
        // B(1) -> C(2)
        store.create_edge(&Relation::new(ids[1], ids[2], 1, true)).unwrap();
        // C(2) -> D(3)
        store.create_edge(&Relation::new(ids[2], ids[3], 1, true)).unwrap();
        // A(0) -> E(4)
        store.create_edge(&Relation::new(ids[0], ids[4], 2, true)).unwrap();

        (store, ids)
    }

    #[test]
    fn test_bfs_basic() {
        let (mut store, ids) = build_test_graph("bfs");
        let off0 = store.find_node_offset(ids[0]).unwrap();

        let result = bfs(&mut store, off0, 2, None).unwrap();
        let depths: Vec<_> = result.iter().map(|(_, d)| *d).collect();
        assert_eq!(depths[0], 0); // A
        assert!(depths.contains(&1)); // B, E
        assert!(depths.contains(&2)); // C
    }

    #[test]
    fn test_dfs_basic() {
        let (mut store, ids) = build_test_graph("dfs");
        let off0 = store.find_node_offset(ids[0]).unwrap();

        let result = dfs(&mut store, off0, 3, None).unwrap();
        assert!(!result.is_empty());
        assert_eq!(result[0].1, 0); // 起点深度为 0
    }

    #[test]
    fn test_pagerank() {
        let (mut store, _ids) = build_test_graph("pr");
        let pr = pagerank(&mut store, 10, 0.85).unwrap();

        // 所有 PR 值之和 ≈ 1
        let sum: f64 = pr.values().sum();
        assert!((sum - 1.0).abs() < 0.01);

        // A 有最多出边，PR 应较高
        // D 没有出边，是 sink
    }

    #[test]
    fn test_shortest_path() {
        let (mut store, ids) = build_test_graph("sp");
        let off0 = store.find_node_offset(ids[0]).unwrap();

        let path = shortest_path(&mut store, off0, ids[3], None).unwrap();
        assert_eq!(path.len(), 4); // A -> B -> C -> D
    }

    #[test]
    fn test_edge_filter() {
        let f = EdgeFilter::by_type(1);
        let mut edge = EdgeRecord::empty();
        edge.relation_type = 1;
        assert!(f.matches(&edge));

        edge.relation_type = 2;
        assert!(!f.matches(&edge));
    }
}
