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

use std::cell::RefCell;

use crate::being::Being;
use crate::column::{AggregateOp, ProjectedLayer};
use crate::error::DaoQLError;
use crate::graph::store::GraphStore;
use crate::id::BeingId;
use crate::index::uuid_index::UuidIndex;
use crate::query::executor::QueryResult;
use crate::version::HistoryMode;
use crate::vector::hnsw::HnswIndex;

/// Query builder
pub struct QueryBuilder<'a> {
    graph: &'a RefCell<GraphStore>,
    vector_indices: &'a std::collections::HashMap<String, HnswIndex>,
    uuid_index: &'a RefCell<UuidIndex>,
    column: &'a RefCell<ProjectedLayer>,
    target: QueryTarget,
    filters: Vec<Filter>,
    limit: Option<usize>,
    with_relations: Option<usize>,
    history: Option<HistoryMode>,
    aggregate: Option<(String, AggregateOp)>,
}

#[derive(Debug, Clone)]
enum QueryTarget {
    Being(BeingId),
    Scan { def: String },
    Similar { vector: Vec<f32>, k: usize },
}

#[derive(Debug, Clone)]
struct Filter {
    field: String,
    op: String,
    value: serde_json::Value,
}

impl<'a> QueryBuilder<'a> {
    pub fn new(graph: &'a RefCell<GraphStore>, vector_indices: &'a std::collections::HashMap<String, HnswIndex>, uuid_index: &'a RefCell<UuidIndex>, column: &'a RefCell<ProjectedLayer>) -> Self {
        Self {
            graph,
            vector_indices,
            uuid_index,
            column,
            target: QueryTarget::Scan { def: String::new() },
            filters: Vec::new(),
            limit: None,
            with_relations: None,
            history: None,
            aggregate: None,
        }
    }

    /// By ID Query（use UUID→Offset B+Tree index）
    pub fn being(mut self, id: BeingId) -> Self {
        self.target = QueryTarget::Being(id);
        self
    }

    /// BytypeScan
    pub fn scan(mut self, def: impl Into<String>) -> Self {
        self.target = QueryTarget::Scan { def: def.into() };
        self
    }

    /// VectorSimilarity search
    pub fn similar_to(mut self, vector: Vec<f32>, k: usize) -> Self {
        self.target = QueryTarget::Similar { vector, k };
        self
    }

    /// Add filter condition
    pub fn filter(mut self, field: impl Into<String>, op: impl Into<String>, value: serde_json::Value) -> Self {
        self.filters.push(Filter {
            field: field.into(),
            op: op.into(),
            value,
        });
        self
    }

    /// Limit count
    pub fn limit(mut self, n: usize) -> Self {
        self.limit = Some(n);
        self
    }

    /// With relation traversal
    pub fn with_relations(mut self, depth: usize) -> Self {
        self.with_relations = Some(depth);
        self
    }

    /// Versionbacktracking
    pub fn history(mut self, mode: HistoryMode) -> Self {
        self.history = Some(mode);
        self
    }

    /// Aggregate query
    pub fn aggregate(mut self, column: impl Into<String>, op: AggregateOp) -> Self {
        self.aggregate = Some((column.into(), op));
        self
    }

    /// ExecuteQuery，routetographengine
    pub fn execute(self) -> Result<QueryResult, DaoQLError> {
        // ── Fast path: scan + single-column numeric filter + aggregate → direct columnar filter aggregate ──
        if self.aggregate.is_some() && self.filters.len() == 1 {
            if let QueryTarget::Scan { ref def } = self.target {
                if def.is_empty() {
                    let (col_name, op) = self.aggregate.clone().unwrap();
                    let filter = &self.filters[0];
                    if filter.field == col_name {
                        if let Some(threshold) = filter.value.as_f64() {
                            let column = self.column.borrow();
                            if let Some(col) = column.column(&col_name) {
                                let (value, count) = match (filter.op.as_str(), op) {
                                    ("gt" | ">", crate::column::aggregation::AggregateOp::Sum) => {
                                        let values = col.values_slice();
                                        let mut sum = 0.0;
                                        let mut c = 0usize;
                                        for &v in values {
                                            let mask = (v > threshold) as u64;
                                            sum += v * mask as f64;
                                            c += mask as usize;
                                        }
                                        (sum, c)
                                    }
                                    ("gte" | ">=", crate::column::aggregation::AggregateOp::Sum) => {
                                        let mut sum = 0.0;
                                        let mut c = 0;
                                        for v in col.values() {
                                            if v >= threshold { sum += v; c += 1; }
                                        }
                                        (sum, c)
                                    }
                                    ("lt" | "<", crate::column::aggregation::AggregateOp::Sum) => {
                                        let mut sum = 0.0;
                                        let mut c = 0;
                                        for v in col.values() {
                                            if v < threshold { sum += v; c += 1; }
                                        }
                                        (sum, c)
                                    }
                                    ("lte" | "<=", crate::column::aggregation::AggregateOp::Sum) => {
                                        let mut sum = 0.0;
                                        let mut c = 0;
                                        for v in col.values() {
                                            if v <= threshold { sum += v; c += 1; }
                                        }
                                        (sum, c)
                                    }
                                    ("gt" | ">", crate::column::aggregation::AggregateOp::Count) => {
                                        let c = col.values().filter(|&v| v > threshold).count();
                                        (c as f64, c)
                                    }
                                    ("gt" | ">", crate::column::aggregation::AggregateOp::Avg) => {
                                        let mut sum = 0.0;
                                        let mut c = 0;
                                        for v in col.values() {
                                            if v > threshold { sum += v; c += 1; }
                                        }
                                        if c > 0 { (sum / c as f64, c) } else { (0.0, 0) }
                                    }
                                    ("gt" | ">", crate::column::aggregation::AggregateOp::Min) => {
                                        let mut min = f64::MAX;
                                        let mut c = 0;
                                        for v in col.values() {
                                            if v > threshold { if v < min { min = v; } c += 1; }
                                        }
                                        if c > 0 { (min, c) } else { (0.0, 0) }
                                    }
                                    ("gt" | ">", crate::column::aggregation::AggregateOp::Max) => {
                                        let mut max = f64::MIN;
                                        let mut c = 0;
                                        for v in col.values() {
                                            if v > threshold { if v > max { max = v; } c += 1; }
                                        }
                                        if c > 0 { (max, c) } else { (0.0, 0) }
                                    }
                                    _ => {
                                        let mut values = Vec::new();
                                        for v in col.values() {
                                            let pass = match filter.op.as_str() {
                                                "==" | "eq" => (v - threshold).abs() < f64::EPSILON,
                                                "!=" | "ne" => (v - threshold).abs() >= f64::EPSILON,
                                                ">" | "gt" => v > threshold,
                                                "<" | "lt" => v < threshold,
                                                ">=" | "gte" => v >= threshold,
                                                "<=" | "lte" => v <= threshold,
                                                _ => true,
                                            };
                                            if pass { values.push(v); }
                                        }
                                        let agg = crate::column::aggregation::aggregate(&values, op)?;
                                        (agg.value, values.len())
                                    }
                                };
                                if count > 0 {
                                    return Ok(QueryResult {
                                        items: Vec::new(),
                                        aggregate_value: Some(value),
                                        aggregate_op: Some(format!("{:?}", op)),
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }

        // ── Fast path: unfiltered full table scan + aggregate → skip graph scan, direct columnar aggregate ──
        if self.filters.is_empty() && self.aggregate.is_some() {
            if let QueryTarget::Scan { ref def } = self.target {
                if def.is_empty() {
                    let (col_name, op) = self.aggregate.clone().unwrap();
                    let column = self.column.borrow();
                    if let Some(col) = column.column(&col_name) {
                        let (value, count) = match op {
                            crate::column::aggregation::AggregateOp::Count => {
                                (col.len() as f64, col.len())
                            }
                            crate::column::aggregation::AggregateOp::Sum => {
                                let values: Vec<f64> = col.values().collect();
                                let sum = crate::column::aggregation::simd_sum(&values);
                                (sum, values.len())
                            }
                            crate::column::aggregation::AggregateOp::Avg => {
                                let values: Vec<f64> = col.values().collect();
                                let sum = crate::column::aggregation::simd_sum(&values);
                                let c = values.len();
                                if c > 0 { (sum / c as f64, c) } else { (0.0, 0) }
                            }
                            crate::column::aggregation::AggregateOp::Min => {
                                let mut min = f64::MAX;
                                let mut c = 0;
                                for v in col.values() {
                                    if v < min { min = v; }
                                    c += 1;
                                }
                                if c > 0 { (min, c) } else { (0.0, 0) }
                            }
                            crate::column::aggregation::AggregateOp::Max => {
                                let mut max = f64::MIN;
                                let mut c = 0;
                                for v in col.values() {
                                    if v > max { max = v; }
                                    c += 1;
                                }
                                if c > 0 { (max, c) } else { (0.0, 0) }
                            }
                            _ => {
                                let values: Vec<f64> = col.values().collect();
                                let agg = crate::column::aggregation::aggregate(&values, op)?;
                                (agg.value, values.len())
                            }
                        };
                        if count > 0 {
                            return Ok(QueryResult {
                                items: Vec::new(),
                                aggregate_value: Some(value),
                                aggregate_op: Some(format!("{:?}", op)),
                            });
                        }
                    }
                }
            }
        }

        let graph = self.graph.borrow();
        let mut items = Vec::new();

        match self.target {
            QueryTarget::Being(id) => {
                let offset = self.uuid_index.borrow().get(id)?;
                if let Some(offset) = offset {
                    if let Ok(node) = graph.read_node(offset) {
                        let being = Being::from_node(node)?;
                        items.push(being);
                    }
                }
            }
            QueryTarget::Scan { def } => {
                for i in 0..graph.node_count() {
                    let offset = (i * crate::graph::record::NodeRecord::SIZE) as u64;
                    if let Ok(node) = graph.read_node(offset) {
                        if node.is_empty() {
                            continue;
                        }
                        if let Ok(being) = Being::from_node(node) {
                            if def.is_empty() || being.core.def == def {
                                items.push(being);
                                if let Some(n) = self.limit {
                                    if items.len() >= n {
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            }
            QueryTarget::Similar { ref vector, k } => {
                if let Some((_, index)) = self.vector_indices.iter().next() {
                    let results = index.search(vector, k)?;
                    for result in results {
                        if let Ok(Some(offset)) = self.uuid_index.borrow().get(result.id) {
                            if let Ok(node) = graph.read_node(offset) {
                                if let Ok(being) = Being::from_node(node) {
                                    items.push(being);
                                }
                            }
                        }
                    }
                }
            }
        }

        // apply filter
        for filter in &self.filters {
            items.retain(|being| {
                let field_value = match filter.field.as_str() {
                    "name" => serde_json::Value::String(being.core.name.clone()),
                    "def" => serde_json::Value::String(being.core.def.clone()),
                    "status" => serde_json::json!(being.core.status),
                    "weight" => serde_json::json!(being.core.weight),
                    "priority" => serde_json::json!(being.core.priority),
                    "category" => serde_json::Value::String(being.core.category.clone()),
                    _ => return true,
                };
                match filter.op.as_str() {
                    "==" | "eq" => field_value == filter.value,
                    "!=" | "ne" => field_value != filter.value,
                    ">" | "gt" => {
                        if let (Some(a), Some(b)) = (field_value.as_f64(), filter.value.as_f64()) {
                            a > b
                        } else {
                            false
                        }
                    }
                    "<" | "lt" => {
                        if let (Some(a), Some(b)) = (field_value.as_f64(), filter.value.as_f64()) {
                            a < b
                        } else {
                            false
                        }
                    }
                    ">=" | "gte" => {
                        if let (Some(a), Some(b)) = (field_value.as_f64(), filter.value.as_f64()) {
                            a >= b
                        } else {
                            false
                        }
                    }
                    "<=" | "lte" => {
                        if let (Some(a), Some(b)) = (field_value.as_f64(), filter.value.as_f64()) {
                            a <= b
                        } else {
                            false
                        }
                    }
                    _ => true,
                }
            });
        }

        // Aggregate mode: for query result set extract value from Being's ProjectedLayer and aggregate
        if let Some((col_name, op)) = self.aggregate {
            let column = self.column.borrow();
            if let Some(col) = column.column(&col_name) {
                // simpleAggregateuse inline accumulators，avoid Vec allocate
                let (value, count) = match op {
                    crate::column::aggregation::AggregateOp::Count => {
                        let c = items.iter().filter(|b| col.get(b.core.id).is_some()).count();
                        (c as f64, c)
                    }
                    crate::column::aggregation::AggregateOp::Sum => {
                        let mut sum = 0.0;
                        let mut c = 0;
                        for being in &items {
                            if let Some(v) = col.get(being.core.id) {
                                sum += v;
                                c += 1;
                            }
                        }
                        (sum, c)
                    }
                    crate::column::aggregation::AggregateOp::Avg => {
                        let mut sum = 0.0;
                        let mut c = 0;
                        for being in &items {
                            if let Some(v) = col.get(being.core.id) {
                                sum += v;
                                c += 1;
                            }
                        }
                        if c > 0 { (sum / c as f64, c) } else { (0.0, 0) }
                    }
                    crate::column::aggregation::AggregateOp::Min => {
                        let mut min = f64::MAX;
                        let mut c = 0;
                        for being in &items {
                            if let Some(v) = col.get(being.core.id) {
                                if v < min { min = v; }
                                c += 1;
                            }
                        }
                        if c > 0 { (min, c) } else { (0.0, 0) }
                    }
                    crate::column::aggregation::AggregateOp::Max => {
                        let mut max = f64::MIN;
                        let mut c = 0;
                        for being in &items {
                            if let Some(v) = col.get(being.core.id) {
                                if v > max { max = v; }
                                c += 1;
                            }
                        }
                        if c > 0 { (max, c) } else { (0.0, 0) }
                    }
                    _ => {
                        // complex aggregate (Median/Stddev/Variance) fallback to Vec path
                        let mut values = Vec::new();
                        for being in &items {
                            if let Some(v) = col.get(being.core.id) {
                                values.push(v);
                            }
                        }
                        let agg_result = crate::column::aggregation::aggregate(&values, op)?;
                        (agg_result.value, values.len())
                    }
                };
                if count > 0 {
                    return Ok(QueryResult {
                        items,
                        aggregate_value: Some(value),
                        aggregate_op: Some(format!("{:?}", op)),
                    });
                }
            }
        }

        Ok(QueryResult { items, aggregate_value: None, aggregate_op: None })
    }

    /// Getsingleresult
    pub fn fetch_one(self) -> Result<Option<Being>, DaoQLError> {
        let mut result = self.execute()?;
        Ok(result.items.pop())
    }

    /// Getallresult
    pub fn fetch_all(self) -> Result<Vec<Being>, DaoQLError> {
        Ok(self.execute()?.items)
    }
}
