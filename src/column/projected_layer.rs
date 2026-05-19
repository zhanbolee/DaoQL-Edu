// Copyright (c) 2026 Zhanbo Li / Atlas Lee <4859345@qq.com>
// SPDX-License-Identifier: BSL-1.1
//
// Licensed under the Business Source License, version 1.1 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at:
//
//     https://spdx.org/licenses/BSL-1.1.html
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
//
//! ProjectedLayer — projected column storage layer
//!
//! Educational Notes:
//! - will hot fields from RawLayer materialize into independent column store
//! - one per column Vec<T>, memory continuous, favorable for CPU cache
//! - cooperate with SIMD implement vectorized aggregate
//! - edu edition simplification: only in-memory store, not persisted to disk

use std::collections::HashMap;

use crate::being::Being;
use crate::error::DaoQLError;
use crate::id::BeingId;

/// Projectcolumn
pub struct ProjectedColumn {
    pub name: String,
    pub values: Vec<f64>,
    pub being_ids: Vec<BeingId>,
}

impl ProjectedColumn {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            values: Vec::new(),
            being_ids: Vec::new(),
        }
    }

    pub fn push(&mut self, id: BeingId, value: f64) {
        self.being_ids.push(id);
        self.values.push(value);
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Find value by BeingId (linear search, edu edition simplification)
    pub fn get(&self, id: BeingId) -> Option<f64> {
        self.being_ids.iter().position(|&bid| bid == id)
            .map(|idx| self.values[idx])
    }

    /// Traverseallvalue（used forAggregate fast path）
    pub fn values(&self) -> impl Iterator<Item = f64> + '_ {
        self.values.iter().copied()
    }

    /// directly access lower layer value slice（used for SIMD batch processing）
    pub fn values_slice(&self) -> &[f64] {
        &self.values
    }
}

/// Projected layer — manage multiple columns
pub struct ProjectedLayer {
    columns: HashMap<String, ProjectedColumn>,
}

impl ProjectedLayer {
    pub fn new() -> Self {
        Self {
            columns: HashMap::new(),
        }
    }

    /// Registerprojectcolumn
    pub fn register(&mut self, name: impl Into<String>) {
        let name = name.into();
        self.columns.entry(name.clone()).or_insert_with(|| ProjectedColumn::new(name));
    }

    /// From Being extract and insert projected column
    pub fn project(&mut self, being: &Being) -> Result<(), DaoQLError> {
        for (col_name, col) in self.columns.iter_mut() {
            if let Some(value) = extract_value(being, col_name) {
                col.push(being.core.id, value);
            }
        }
        Ok(())
    }

    /// Get column
    pub fn column(&self, name: &str) -> Option<&ProjectedColumn> {
        self.columns.get(name)
    }

    /// Get column（mutable）
    pub fn column_mut(&mut self, name: &str) -> Option<&mut ProjectedColumn> {
        self.columns.get_mut(name)
    }

    /// all column names
    pub fn column_names(&self) -> Vec<&str> {
        self.columns.keys().map(|s| s.as_str()).collect()
    }

    /// Total record count (take first column length)
    pub fn row_count(&self) -> usize {
        self.columns.values().next().map(|c| c.len()).unwrap_or(0)
    }
}

impl Default for ProjectedLayer {
    fn default() -> Self {
        Self::new()
    }
}

/// Extract numeric value from Being (edu edition simplification: only some fields)
fn extract_value(being: &Being, field: &str) -> Option<f64> {
    match field {
        "weight" => Some(being.core.weight),
        "priority" => Some(being.core.priority as f64),
        _ => {
            // try secondary dynamic field extract
            being.ext.as_ref()?.dynamic_attrs.get(field)
                .and_then(|v| v.as_f64())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_projected_layer_basic() {
        let mut layer = ProjectedLayer::new();
        layer.register("weight");
        layer.register("priority");

        let being = Being::new("Test", "Def");
        layer.project(&being).unwrap();

        let weight_col = layer.column("weight").unwrap();
        assert_eq!(weight_col.len(), 1);
        assert_eq!(weight_col.values[0], 0.0);
    }

    #[test]
    fn test_projected_layer_dynamic() {
        let mut layer = ProjectedLayer::new();
        layer.register("price");

        let being = Being::new("Product", "ProductDef")
            .with_attr("price", serde_json::json!(99.99));
        layer.project(&being).unwrap();

        let col = layer.column("price").unwrap();
        assert_eq!(col.values[0], 99.99);
    }
}
