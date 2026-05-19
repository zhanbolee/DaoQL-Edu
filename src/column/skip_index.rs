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


/// Granule metadata
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GranuleMeta {
    /// Dataoffset
    pub offset: u64,
    /// Recordcount
    pub count: u32,
    /// Minimum (8 bytes, enough to store i64/f64)
    pub min_val: [u8; 8],
    /// Maximum
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

    /// Set min/max（f64）
    pub fn set_f64(&mut self, min: f64, max: f64) {
        self.min_val = min.to_le_bytes();
        self.max_val = max.to_le_bytes();
    }

    /// Get min（f64）
    pub fn min_f64(&self) -> f64 {
        f64::from_le_bytes(self.min_val)
    }

    /// Get max（f64）
    pub fn max_f64(&self) -> f64 {
        f64::from_le_bytes(self.max_val)
    }

    /// Check value whether in range
    pub fn contains_f64(&self, value: f64) -> bool {
        value >= self.min_f64() && value <= self.max_f64()
    }

    /// Check range whether overlap
    pub fn overlaps_f64(&self, min: f64, max: f64) -> bool {
        !(self.max_f64() < min || self.min_f64() > max)
    }
}

/// Skip Index — Granule metadatalist
pub struct SkipIndex {
    pub granules: Vec<GranuleMeta>,
}

impl SkipIndex {
    pub fn new() -> Self {
        Self { granules: Vec::new() }
    }

    /// Add Granule metadata
    pub fn push(&mut self, meta: GranuleMeta) {
        self.granules.push(meta);
    }

    /// Query: return possible containing value Granule index list
    pub fn query_f64(&self, min: f64, max: f64) -> Vec<usize> {
        self.granules
            .iter()
            .enumerate()
            .filter(|(_, g)| g.overlaps_f64(min, max))
            .map(|(i, _)| i)
            .collect()
    }

    /// Statistics pruning effect
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

        // Query [15, 22], should match g2 and g3
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
