// Copyright 2026 Zhanbo Li / Atlas Lee <zhanbo.lee@hotmail.com>
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

use crate::error::DaoQLError;
use crate::id::BeingId;
use crate::vector::hnsw::HnswIndex;

/// Graph prior seed selection
pub struct GraphPrior;

impl GraphPrior {
    /// Select HNSW insert seed node
    ///
    /// Policy：
    /// 1. if adjacent nodes already exist in HNSW, return these nodes as seeds
    /// 2. otherwise, return global entry point
    pub fn select_seed(
        _hnsw: &HnswIndex,
        _being_id: BeingId,
        _neighbors: &[BeingId],
    ) -> Result<Vec<BeingId>, DaoQLError> {
        // edu edition simplification: return adjacent nodes directly
        // production version should check whether node already in HNSW
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
