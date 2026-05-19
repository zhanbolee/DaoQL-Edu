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
