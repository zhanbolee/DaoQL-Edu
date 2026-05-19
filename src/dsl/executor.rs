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

use std::cell::RefCell;
use std::collections::HashMap;

use crate::dsl::ast::{DslQuery, MutationOp, FilterExpr};
use crate::error::DaoQLError;
use crate::column::ProjectedLayer;
use crate::graph::record::NodeRecord;
use crate::graph::store::GraphStore;
use crate::index::uuid_index::UuidIndex;
use crate::vector::hnsw::HnswIndex;

/// Evaluate FilterExpr whether holds for given Being
fn evaluate_filter(being: &crate::being::Being, filter: &FilterExpr) -> bool {
    match filter {
        FilterExpr::Eq { field, value } => match field.as_str() {
            "name" => being.core.name == value.as_str().unwrap_or(""),
            "def" => being.core.def == value.as_str().unwrap_or(""),
            "status" => value.as_u64().is_some_and(|v| being.core.status as u64 == v),
            "weight" => value.as_f64().is_some_and(|v| (being.core.weight - v).abs() < f64::EPSILON),
            "priority" => value.as_u64().is_some_and(|v| being.core.priority as u64 == v),
            "category" => being.core.category == value.as_str().unwrap_or(""),
            _ => true,
        },
        FilterExpr::Gt { field, value } => match field.as_str() {
            "weight" => value.as_f64().is_some_and(|v| being.core.weight > v),
            "status" => value.as_u64().is_some_and(|v| being.core.status as u64 > v),
            "priority" => value.as_u64().is_some_and(|v| being.core.priority as u64 > v),
            _ => false,
        },
        FilterExpr::Lt { field, value } => match field.as_str() {
            "weight" => value.as_f64().is_some_and(|v| being.core.weight < v),
            "status" => value.as_u64().is_some_and(|v| (being.core.status as u64) < v),
            "priority" => value.as_u64().is_some_and(|v| (being.core.priority as u64) < v),
            _ => false,
        },
        FilterExpr::Gte { field, value } => match field.as_str() {
            "weight" => value.as_f64().is_some_and(|v| being.core.weight >= v),
            "status" => value.as_u64().is_some_and(|v| being.core.status as u64 >= v),
            "priority" => value.as_u64().is_some_and(|v| being.core.priority as u64 >= v),
            _ => false,
        },
        FilterExpr::Lte { field, value } => match field.as_str() {
            "weight" => value.as_f64().is_some_and(|v| being.core.weight <= v),
            "status" => value.as_u64().is_some_and(|v| being.core.status as u64 <= v),
            "priority" => value.as_u64().is_some_and(|v| being.core.priority as u64 <= v),
            _ => false,
        },
        FilterExpr::And(left, right) => evaluate_filter(being, left) && evaluate_filter(being, right),
        FilterExpr::Or(left, right) => evaluate_filter(being, left) || evaluate_filter(being, right),
    }
}

/// DSL Execute
pub struct DslExecutor;

impl DslExecutor {
    pub fn new() -> Self {
        Self
    }

    /// Execute DSL query
    pub fn execute(
        &self,
        query: &DslQuery,
        graph: &RefCell<GraphStore>,
        def_registry: &RefCell<crate::def::DefRegistry>,
        vector_indices: &HashMap<String, HnswIndex>,
        uuid_index: &RefCell<UuidIndex>,
        column: &RefCell<ProjectedLayer>,
    ) -> Result<DslResult, DaoQLError> {
        match query {
            DslQuery::Query { target, filter, projections, limit, history: _, aggregate } => {
                // if has aggregate, go column store aggregate fast path
                if let Some((col_name, op_name)) = aggregate {
                    let agg_op = match op_name.as_str() {
                        "count" | "COUNT" => crate::column::AggregateOp::Count,
                        "sum" | "SUM" => crate::column::AggregateOp::Sum,
                        "avg" | "AVG" => crate::column::AggregateOp::Avg,
                        "min" | "MIN" => crate::column::AggregateOp::Min,
                        "max" | "MAX" => crate::column::AggregateOp::Max,
                        _ => crate::column::AggregateOp::Sum,
                    };

                    let col = column.borrow();
                    if let Some(projected_col) = col.column(col_name) {
                        let (value, count) = if filter.is_some() {
                            let graph = graph.borrow();
                            let mut filtered_values = Vec::new();
                            for i in 0..graph.node_count() {
                                let offset = (i * NodeRecord::SIZE) as u64;
                                if let Ok(node) = graph.read_node(offset) {
                                    if node.is_empty() {
                                        continue;
                                    }
                                    if let Ok(being) = crate::being::Being::from_node(node) {
                                        if (target.is_empty() || being.core.def == *target)
                                            && filter.as_ref().map_or(true, |f| evaluate_filter(&being, f)) {
                                                if let Some(v) = projected_col.get(being.core.id) {
                                                    filtered_values.push(v);
                                                }
                                            }
                                    }
                                }
                            }
                            let value = match agg_op {
                                crate::column::AggregateOp::Count => filtered_values.len() as f64,
                                crate::column::AggregateOp::Sum => crate::column::aggregation::simd_sum(&filtered_values),
                                crate::column::AggregateOp::Avg => {
                                    if filtered_values.is_empty() {
                                        0.0
                                    } else {
                                        crate::column::aggregation::simd_sum(&filtered_values) / filtered_values.len() as f64
                                    }
                                }
                                crate::column::AggregateOp::Min => filtered_values.iter().fold(f64::INFINITY, |a, &b| a.min(b)),
                                crate::column::AggregateOp::Max => filtered_values.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b)),
                                _ => crate::column::aggregation::simd_sum(&filtered_values),
                            };
                            (value, filtered_values.len())
                        } else {
                            let values: Vec<f64> = projected_col.values().collect();
                            match agg_op {
                                crate::column::AggregateOp::Count => (values.len() as f64, values.len()),
                                crate::column::AggregateOp::Sum => (crate::column::aggregation::simd_sum(&values), values.len()),
                                crate::column::AggregateOp::Avg => {
                                    if values.is_empty() {
                                        (0.0, 0)
                                    } else {
                                        (crate::column::aggregation::simd_sum(&values) / values.len() as f64, values.len())
                                    }
                                }
                                crate::column::AggregateOp::Min => {
                                    let v = values.iter().fold(f64::INFINITY, |a, &b| a.min(b));
                                    (v, values.len())
                                }
                                crate::column::AggregateOp::Max => {
                                    let v = values.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
                                    (v, values.len())
                                }
                                _ => (crate::column::aggregation::simd_sum(&values), values.len()),
                            }
                        };
                        return Ok(DslResult::Data(vec![serde_json::json!({
                            "aggregate": value,
                            "count": count,
                            "op": op_name,
                        })]));
                    }
                }

                let graph = graph.borrow();
                let mut items = Vec::new();
                for i in 0..graph.node_count() {
                    let offset = (i * NodeRecord::SIZE) as u64;
                    if let Ok(node) = graph.read_node(offset) {
                        if node.is_empty() {
                            continue;
                        }
                        if let Ok(being) = crate::being::Being::from_node(node) {
                            if (target.is_empty() || being.core.def == *target)
                                && filter.as_ref().map_or(true, |f| evaluate_filter(&being, f)) {
                                    let mut obj = serde_json::Map::new();
                                    obj.insert("id".to_string(), serde_json::json!(being.core.id.to_string()));
                                    if projections.is_empty() || projections.contains(&"name".to_string()) {
                                        obj.insert("name".to_string(), serde_json::json!(being.core.name));
                                    }
                                    if projections.is_empty() || projections.contains(&"def".to_string()) {
                                        obj.insert("def".to_string(), serde_json::json!(being.core.def));
                                    }
                                    if projections.is_empty() || projections.contains(&"status".to_string()) {
                                        obj.insert("status".to_string(), serde_json::json!(being.core.status));
                                    }
                                    if projections.is_empty() || projections.contains(&"weight".to_string()) {
                                        obj.insert("weight".to_string(), serde_json::json!(being.core.weight));
                                    }
                                    if projections.is_empty() || projections.contains(&"priority".to_string()) {
                                        obj.insert("priority".to_string(), serde_json::json!(being.core.priority));
                                    }
                                    if projections.is_empty() || projections.contains(&"category".to_string()) {
                                        obj.insert("category".to_string(), serde_json::json!(being.core.category));
                                    }
                                    items.push(serde_json::Value::Object(obj));
                                    if let Some(n) = limit {
                                        if items.len() >= *n {
                                            break;
                                        }
                                    }
                                }
                        }
                    }
                }
                Ok(DslResult::Data(items))
            }
            DslQuery::Mutation { op, target, input } => {
                match op {
                    MutationOp::Create => {
                        let mut def_registry = def_registry.borrow_mut();
                        let def = crate::def::Def::new(target);
                        let def_type_code = def_registry.register(def);
                        drop(def_registry);
                        let mut graph = graph.borrow_mut();
                        let mut core = crate::being::BeingCore::new(target, target);
                        core.name = input.iter().find(|(k, _)| k == "name")
                            .map(|(_, v)| v.as_str().unwrap_or("").to_string())
                            .unwrap_or_else(|| target.clone());
                        let offset = graph.create_node(&core, def_type_code)?;
                        drop(graph);
                        let uuid_index = uuid_index.borrow();
                        uuid_index.insert(core.id, offset)?;
                        drop(uuid_index);
                        let being = crate::being::Being { core, ext: None, embeddings: HashMap::new() };
                        column.borrow_mut().project(&being)?;
                        Ok(DslResult::Message(format!("Create {target}")))
                    }
                    MutationOp::Update => {
                        let mut graph = graph.borrow_mut();
                        for i in 0..graph.node_count() {
                            let offset = (i * NodeRecord::SIZE) as u64;
                            if let Ok(node) = graph.read_node(offset) {
                                if node.get_name() == *target {
                                    let node_mut = graph.read_node_mut(offset)?;
                                    for (k, v) in input {
                                        if k == "name" {
                                            node_mut.set_name(v.as_str().unwrap_or(""));
                                        } else if k == "description" {
                                            node_mut.set_description(v.as_str().unwrap_or(""));
                                        }
                                    }
                                    break;
                                }
                            }
                        }
                        Ok(DslResult::Message(format!("Update {target}")))
                    }
                    MutationOp::Delete => {
                        let mut graph = graph.borrow_mut();
                        for i in 0..graph.node_count() {
                            let offset = (i * NodeRecord::SIZE) as u64;
                            if let Ok(node) = graph.read_node(offset) {
                                if node.get_name() == *target {
                                    let node_mut = graph.read_node_mut(offset)?;
                                    node_mut.status = 0;
                                    break;
                                }
                            }
                        }
                        Ok(DslResult::Message(format!("Delete {target}")))
                    }
                }
            }
            DslQuery::Analyze { algorithm, target, limit } => {
                let mut graph_mut = graph.borrow_mut();
                let k = limit.unwrap_or(10);
                // in graph engine find target corresponding to start node
                let mut start_offset = None;
                for i in 0..graph_mut.node_count() {
                    let offset = (i * NodeRecord::SIZE) as u64;
                    if let Ok(node) = graph_mut.read_node(offset) {
                        if !node.is_empty() && node.get_name() == *target {
                            start_offset = Some(offset);
                            break;
                        }
                    }
                }
                let results = match algorithm.as_str() {
                    "pagerank" => {
                        let ranks = crate::graph::traversal::pagerank(&mut graph_mut, k, 0.85)?;
                        ranks.into_iter().map(|(id, rank)| {
                            serde_json::json!({"id": id.to_string(), "rank": rank})
                        }).collect::<Vec<_>>()
                    }
                    "bfs" => {
                        if let Some(start) = start_offset {
                            let visited = crate::graph::traversal::bfs(&mut graph_mut, start, k, None)?;
                            drop(graph_mut);
                            let graph = graph.borrow();
                            visited.into_iter().map(|(offset, depth)| {
                                let mut obj = serde_json::json!({"offset": offset, "depth": depth});
                                if let Ok(node) = graph.read_node(offset) {
                                    if let Ok(being) = crate::being::Being::from_node(node) {
                                        obj["id"] = serde_json::json!(being.core.id.to_string());
                                        obj["name"] = serde_json::json!(being.core.name);
                                        let col = column.borrow();
                                        if let Some(w) = col.column("weight").and_then(|c| c.get(being.core.id)) {
                                            obj["weight"] = serde_json::json!(w);
                                        }
                                        if let Some(p) = col.column("priority").and_then(|c| c.get(being.core.id)) {
                                            obj["priority"] = serde_json::json!(p);
                                        }
                                    }
                                }
                                obj
                            }).collect::<Vec<_>>()
                        } else {
                            Vec::new()
                        }
                    }
                    "dfs" => {
                        if let Some(start) = start_offset {
                            let visited = crate::graph::traversal::dfs(&mut graph_mut, start, k, None)?;
                            drop(graph_mut);
                            let graph = graph.borrow();
                            visited.into_iter().map(|(offset, depth)| {
                                let mut obj = serde_json::json!({"offset": offset, "depth": depth});
                                if let Ok(node) = graph.read_node(offset) {
                                    if let Ok(being) = crate::being::Being::from_node(node) {
                                        obj["id"] = serde_json::json!(being.core.id.to_string());
                                        obj["name"] = serde_json::json!(being.core.name);
                                        let col = column.borrow();
                                        if let Some(w) = col.column("weight").and_then(|c| c.get(being.core.id)) {
                                            obj["weight"] = serde_json::json!(w);
                                        }
                                        if let Some(p) = col.column("priority").and_then(|c| c.get(being.core.id)) {
                                            obj["priority"] = serde_json::json!(p);
                                        }
                                    }
                                }
                                obj
                            }).collect::<Vec<_>>()
                        } else {
                            Vec::new()
                        }
                    }
                    _ => Vec::new(),
                };
                Ok(DslResult::Data(results))
            }
            DslQuery::Define { name, fields } => {
                let mut def_registry = def_registry.borrow_mut();
                let mut def = crate::def::Def::new(name);
                for fd in fields {
                    let ft = match fd.field_type.as_str() {
                        "String" => crate::def::FieldType::String,
                        "Int" => crate::def::FieldType::Int,
                        "Float" => crate::def::FieldType::Float,
                        "Bool" => crate::def::FieldType::Bool,
                        _ => crate::def::FieldType::String,
                    };
                    let field = crate::def::Field::new(&fd.name, ft);
                    let field = if fd.required { field.required() } else { field };
                    def = def.with_field(field);
                }
                def_registry.register(def);
                Ok(DslResult::Message(format!("Define {name}")))
            }
            DslQuery::Similar { target: _target, query_vector, k } => {
                let graph = graph.borrow();
                let mut items = Vec::new();
                // Take first available vector index to execute search
                if let Some((_, index)) = vector_indices.iter().next() {
                    let results = index.search(query_vector, *k)?;
                    for result in results {
                        if let Ok(Some(offset)) = uuid_index.borrow().get(result.id) {
                            // Collect relation info first (avoid borrow conflict with subsequent read_node)
                            let mut related = Vec::new();
                            if let Ok(edges) = graph.out_edges(offset) {
                                let edge_targets: Vec<_> = edges.iter().map(|e| e.to_id).collect();
                                for to_id in edge_targets {
                                    if let Ok(to_offset) = graph.find_node_offset(to_id) {
                                        if let Ok(target_node) = graph.read_node(to_offset) {
                                            if let Ok(target_being) = crate::being::Being::from_node(target_node) {
                                                related.push(target_being.core.name);
                                            }
                                        }
                                    }
                                }
                            }

                            if let Ok(node) = graph.read_node(offset) {
                                if let Ok(being) = crate::being::Being::from_node(node) {
                                    let mut obj = serde_json::Map::new();
                                    obj.insert("id".to_string(), serde_json::json!(being.core.id.to_string()));
                                    obj.insert("name".to_string(), serde_json::json!(being.core.name));
                                    obj.insert("def".to_string(), serde_json::json!(being.core.def));
                                    obj.insert("distance".to_string(), serde_json::json!(result.distance));

                                    // read column store attribute
                                    let col = column.borrow();
                                    if let Some(w) = col.column("weight").and_then(|c| c.get(being.core.id)) {
                                        obj.insert("weight".to_string(), serde_json::json!(w));
                                    }
                                    if let Some(p) = col.column("priority").and_then(|c| c.get(being.core.id)) {
                                        obj.insert("priority".to_string(), serde_json::json!(p));
                                    }
                                    drop(col);

                                    if !related.is_empty() {
                                        obj.insert("relation_count".to_string(), serde_json::json!(related.len()));
                                        obj.insert("relations".to_string(), serde_json::json!(related));
                                    }

                                    items.push(serde_json::Value::Object(obj));
                                }
                            }
                        }
                    }
                }
                Ok(DslResult::Data(items))
            }
        }
    }
}

impl Default for DslExecutor {
    fn default() -> Self {
        Self::new()
    }
}

/// DSL Executeresult
#[derive(Debug, Clone)]
pub enum DslResult {
    Message(String),
    Data(Vec<serde_json::Value>),
}
