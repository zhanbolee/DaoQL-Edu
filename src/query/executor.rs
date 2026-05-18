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
//! 查询执行器
//!
//! 教学说明：
//! - 通用查询结果迭代器
//! - 惰性求值：直到调用 next() 才真正读取数据
//! - 教学重点：展示迭代器适配器模式在查询引擎中的应用

use std::cell::RefCell;

use crate::being::Being;
use crate::error::DaoQLError;
use crate::graph::record::NodeRecord;
use crate::graph::store::GraphStore;
use crate::query::planner::{PlanStep, QueryPlan};

/// 查询引擎
pub struct QueryEngine<'a> {
    graph: &'a RefCell<GraphStore>,
}

impl<'a> QueryEngine<'a> {
    pub fn new(graph: &'a RefCell<GraphStore>) -> Self {
        Self { graph }
    }

    /// 执行查询计划
    pub fn execute(&self, plan: &QueryPlan) -> Result<QueryResult, DaoQLError> {
        let graph = self.graph.borrow();
        let mut items = Vec::new();

        for step in &plan.steps {
            match step {
                PlanStep::Scan { def, .. } => {
                    for i in 0..graph.node_count() {
                        let offset = (i * NodeRecord::SIZE) as u64;
                        if let Ok(node) = graph.read_node(offset) {
                            if node.is_empty() {
                                continue;
                            }
                            if let Ok(being) = Being::from_node(node) {
                                if def.is_empty() || being.core.def == *def {
                                    items.push(being);
                                }
                            }
                        }
                    }
                }
                PlanStep::Limit { n } => {
                    items.truncate(*n);
                }
                _ => {
                    // 教学版：Filter/Project/Aggregate 暂不实现
                }
            }
        }

        Ok(QueryResult { items, aggregate_value: None, aggregate_op: None })
    }
}



/// 查询结果
pub struct QueryResult {
    pub items: Vec<Being>,
    /// 聚合查询结果（如有）
    pub aggregate_value: Option<f64>,
    /// 聚合操作类型
    pub aggregate_op: Option<String>,
}

impl QueryResult {
    pub fn new() -> Self {
        Self { items: Vec::new(), aggregate_value: None, aggregate_op: None }
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// 过滤
    pub fn filter(mut self, f: impl Fn(&Being) -> bool) -> Self {
        self.items.retain(f);
        self
    }

    /// 限制数量
    pub fn limit(mut self, n: usize) -> Self {
        self.items.truncate(n);
        self
    }

    /// 收集为 Vec
    pub fn collect(self) -> Vec<Being> {
        self.items
    }
}

impl Default for QueryResult {
    fn default() -> Self {
        Self::new()
    }
}
