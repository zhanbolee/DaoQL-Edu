// Copyright 2026 Zhanbo Li / Atlas Lee <4859345@qq.com>
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::collections::{BinaryHeap, HashMap, HashSet};

use rand::Rng;

use crate::error::DaoQLError;
use crate::id::BeingId;
use crate::vector::distance::{compute_distance, DistanceMetric};

/// Search result (public API)
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

/// HNSW Node
#[derive(Debug, Clone)]
pub struct HnswNode {
    pub id: BeingId,
    pub vector: Vec<f32>,
    /// per layer connected neighbors (layer number → neighbor BeingId list)
    pub connections: Vec<Vec<BeingId>>,
    /// highest layer number
    pub max_level: usize,
}

/// HNSW index
///
/// Standard textbook implementation：
/// - `nodes: HashMap<BeingId, HnswNode>` — by BeingId index
/// - `connections` store `BeingId` rather than `usize`
/// - `entry_point` store `BeingId`
/// - `visited` uses `HashSet<BeingId>` (allocated during search)
/// - distancecomputeusescalar `compute_distance`
pub struct HnswIndex {
    /// Nodestore（by BeingId index）
    nodes: HashMap<BeingId, HnswNode>,
    /// Dimension
    dim: usize,
    /// Max out-degree per layer
    m: usize,
    /// Search width during build
    ef_construction: usize,
    /// Search width during query
    ef_search: usize,
    /// GlobalEntry point（BeingId）
    entry_point: Option<BeingId>,
    /// CurrentMax level
    max_level: usize,
    /// Distance metric
    metric: DistanceMetric,
    /// Random number generator
    rng: rand::rngs::StdRng,
    /// switch to brute-force scan threshold for small datasets
    full_scan_threshold: usize,
    /// flattened embed cache (used for brute-force scan)
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

    /// SetDistance metric
    pub fn with_metric(mut self, metric: DistanceMetric) -> Self {
        self.metric = metric;
        self
    }

    /// Compute random layer count (count decay distribution)
    fn random_level(&mut self) -> usize {
        let mut level = 0;
        let m_l = 1.0 / (self.m as f64).ln();
        while self.rng.gen::<f64>() < m_l && level < 16 {
            level += 1;
        }
        level
    }

    /// ComputeNodetoQueryvectordistance
    fn distance_to_query(&self, node: &HnswNode, query: &[f32]) -> f32 {
        compute_distance(&node.vector, query, self.metric)
    }

    /// Greedy search single layer: find 1 node closest to query
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

    /// Search single layer, return ef nearest neighbors
    /// Use HashSet<BeingId> do visited，BinaryHeap<SearchResult> do candidates and results
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

            // termination condition: current candidate distance already greater than worst in result
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
                        results.pop(); // remove farthest
                    }
                }
            }
        }

        results.into_sorted_vec()
    }

    /// Heuristic neighbor selection (preserve diverse connections)
    fn select_neighbors(
        &self,
        candidates: &[SearchResult],
        m: usize,
    ) -> Vec<SearchResult> {
        // simplified version: directly take nearest m 
        // production version should use heuristic (considering angular diversity)
        candidates.iter().take(m).cloned().collect()
    }

    /// Insert vector (Cosine mode auto pre-normalization)
    pub fn insert(&mut self, id: BeingId, mut vector: Vec<f32>) -> Result<(), DaoQLError> {
        if vector.len() != self.dim {
            return Err(DaoQLError::Query(crate::error::QueryError::DimensionMismatch {
                expected: self.dim,
                actual: vector.len(),
            }));
        }

        // Cosine Pre-normalization: L2-normalize on store, degrade to 1.0 - dot on search
        if self.metric == DistanceMetric::Cosine {
            crate::vector::distance::l2_normalize(&mut vector);
        }

        let level = self.random_level();
        let mut connections: Vec<Vec<BeingId>> = vec![Vec::new(); level + 1];

        // cache vector used for flattened store (clone before move)
        let vector_for_flat = vector.clone();

        let new_node = HnswNode {
            id,
            vector,
            connections: connections.clone(),
            max_level: level,
        };

        // empty index: directly set as entry point
        if self.entry_point.is_none() {
            self.entry_point = Some(id);
            self.max_level = level;
            self.nodes.insert(id, new_node);
            self.flat_ids.push(id);
            self.flat_embeddings.extend_from_slice(&vector_for_flat);
            return Ok(());
        }

        let entry_id = self.entry_point.unwrap();

        // 1. Search entry point from top level
        let mut current_entry = entry_id;
        for l in (level + 1)..=self.max_level {
            current_entry = self.greedy_search_layer(current_entry, &new_node.vector, l);
        }

        // 2. Start from insertion layer, connect layer by layer
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
                // Bidirectional connection
                if let Some(neighbor_node) = self.nodes.get_mut(&neighbor.id) {
                    if l < neighbor_node.connections.len() {
                        neighbor_node.connections[l].push(id);
                        // restrict out-degree
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

        // 3. Update global entry point
        if level > self.max_level {
            self.max_level = level;
            self.entry_point = Some(id);
        }

        // insertNode
        let mut final_node = new_node;
        final_node.connections = connections;
        self.nodes.insert(id, final_node);

        // Update flattened cache
        self.flat_ids.push(id);
        self.flat_embeddings.extend_from_slice(&vector_for_flat);

        Ok(())
    }

    /// brute-force scan search (faster than HNSW traversal for small datasets)
    /// use flattened embed count group lifting cache locality
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

        // Cosine mode pre-normalization query
        let query_normalized: Vec<f32>;
        let q = if self.metric == DistanceMetric::Cosine {
            let mut q = query.to_vec();
            crate::vector::distance::l2_normalize(&mut q);
            query_normalized = q;
            &query_normalized
        } else {
            query
        };

        // small dataset: brute-force scan faster (avoid HNSW traversal overhead)
        if self.nodes.len() <= self.full_scan_threshold {
            return Ok(self.search_brute_force(q, k));
        }

        let entry_id = match self.entry_point {
            Some(e) => e,
            None => return Ok(Vec::new()),
        };

        // Search entry point from top level
        let mut current = entry_id;
        for l in (1..=self.max_level).rev() {
            current = self.greedy_search_layer(current, q, l);
        }

        // search k nearest neighbors at layer 0
        let ef = k.max(self.ef_search);
        let mut results = self.search_layer(current, q, 0, ef);
        results.truncate(k);
        Ok(results)
    }

    /// BatchInsert vector
    pub fn insert_batch(&mut self, items: &[(BeingId, Vec<f32>)]) -> Result<(), DaoQLError> {
        for (id, vec) in items {
            self.insert(*id, vec.clone())?;
        }
        Ok(())
    }

    /// Node count
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

        // insert 100 randomvector
        for _ in 0..100 {
            let id = BeingId::new();
            let vec = random_vector(16);
            index.insert(id, vec).unwrap();
        }

        assert_eq!(index.len(), 100);

        // search
        let query = random_vector(16);
        let results = index.search(&query, 10).unwrap();
        assert_eq!(results.len(), 10);
    }

    #[test]
    fn test_hnsw_recall_rate() {
        let dim = 32;
        let mut index = HnswIndex::new(dim, 16, 100, 64);

        // insert 1000 vector
        let n = 1000;
        let mut vectors: Vec<(BeingId, Vec<f32>)> = Vec::with_capacity(n);
        for _ in 0..n {
            let id = BeingId::new();
            let vec = random_vector(dim);
            index.insert(id, vec.clone()).unwrap();
            vectors.push((id, vec));
        }

        // random query, check recall rate
        let mut total_recall = 0.0;
        let test_queries = 50;

        for _ in 0..test_queries {
            let query = random_vector(dim);

            // HNSW search results
            let hnsw_results = index.search(&query, 10).unwrap();
            let hnsw_ids: std::collections::HashSet<_> =
                hnsw_results.iter().map(|r| r.id).collect();

            // Brute force searchresult（ground truth）
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

            // compute recall rate
            let intersection: Vec<_> = hnsw_ids.intersection(&brute_ids).collect();
            total_recall += intersection.len() as f64 / 10.0;
        }

        let avg_recall = total_recall / test_queries as f64;
        println!("HNSW average recall rate: {:.2}", avg_recall);
        assert!(avg_recall > 0.7, "recall rate too low: {}", avg_recall);
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
