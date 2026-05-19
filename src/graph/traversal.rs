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
//! Graph Traversal Algorithms
//!
//! Educational Notes:
//! - BFS (breadth-first search): level-order traversal, suitable for shortest path
//! - DFS (depth-first search): recursive/stack traversal, suitable for connected components
//! - PageRank: classic graph algorithm, measuring node importance
//!
//! all algorithms based on index-free adjacency linked list, time complexity：
//! - BFS/DFS: O(V + E)
//! - PageRank: O(iterations × E)

use std::collections::{HashMap, HashSet, VecDeque};

use crate::error::DaoQLError;
use crate::graph::record::{EdgeRecord, NodeRecord};
use crate::graph::store::GraphStore;
use crate::id::{BeingId, NodeOffset};

/// EdgeFilter
#[derive(Clone)]
pub struct EdgeFilter {
    /// Limit relation type (None = All)
    pub relation_type: Option<u16>,
    /// Limit directed/undirected (None = All)
    pub directed: Option<bool>,
    /// max weight
    pub max_weight: Option<f64>,
}

impl EdgeFilter {
    /// accept all edges
    pub fn all() -> Self {
        Self {
            relation_type: None,
            directed: None,
            max_weight: None,
        }
    }

    /// ByRelation typeFilter
    pub fn by_type(code: u16) -> Self {
        Self {
            relation_type: Some(code),
            directed: None,
            max_weight: None,
        }
    }

    /// Check whether match
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

/// BFS traversal
///
/// Parameter：
/// - start: start node offset
/// - depth: max depth (0 = only start node)
/// - filter: edge filter
///
/// Return: list of (node offset, depth) in traversal order
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
            // find target node offset
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

/// DFS traversal
///
/// Parameter：
/// - start: start node offset
/// - depth: max depth
/// - filter: edge filter
///
/// Return: list of (node offset, depth) in traversal order
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

/// PageRank algorithm
///
/// Educational Notes:
/// - PageRank is Google searchenginecorealgorithm
/// - idea: a node's importance = sum of all nodes pointing to it (weighted)
/// - formula：PR(u) = (1-d)/N + d × Σ(PR(v)/L(v))
///   - d: damping factor（typically 0.85）
///   - N: totalNodecount
///   - L(v): v outgoing edge count
///   - Σ: for all pointing to u node v sum
///
/// Parameter：
/// - iterations: iteration count
/// - damping: damping factor
///
/// Return：BeingId → PageRank value
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

    // Initialize: all nodes PR = 1/N
    let mut pr: HashMap<BeingId, f64> = HashMap::new();
    for i in 0..n {
        let offset = (i * NodeRecord::SIZE) as u64;
        let node = store.read_node(offset)?;
        pr.insert(node.id, 1.0 / n_f64);
    }

    // iteration
    for _ in 0..iterations {
        let mut new_pr: HashMap<BeingId, f64> = HashMap::new();

        // compute all sink nodes（no outgoing edges） PR total sum，re-allocate to all nodes
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

            // get all incoming edges
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

/// Findshortest path（BFS）
///
/// Return path node offset list（contains start and end）
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
            // rebuildpath
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

    Err(DaoQLError::InvalidState("path does not exist".to_string()))
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

        // Create 5 nodes: A -> B -> C -> D, A -> E
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
        assert_eq!(result[0].1, 0); // start depth is 0
    }

    #[test]
    fn test_pagerank() {
        let (mut store, _ids) = build_test_graph("pr");
        let pr = pagerank(&mut store, 10, 0.85).unwrap();

        // all PR value sum ≈ 1
        let sum: f64 = pr.values().sum();
        assert!((sum - 1.0).abs() < 0.01);

        // A has most outgoing edges, PR should be high
        // D has no outgoing edges, is a sink
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
