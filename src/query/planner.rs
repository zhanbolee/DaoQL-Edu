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
//! 查询计划器
//!
//! 教学说明：
//! - 简单查询计划：解析查询 → 选择引擎 → 生成执行步骤
//! - 教学版不做复杂优化（如谓词下推、连接重排）


/// 查询计划
#[derive(Debug, Clone)]
pub struct QueryPlan {
    pub steps: Vec<PlanStep>,
}

/// 计划步骤
#[derive(Debug, Clone)]
pub enum PlanStep {
    Scan { def: String, filter: Option<String> },
    Filter { predicate: String },
    Project { fields: Vec<String> },
    Aggregate { op: String, field: String },
    Limit { n: usize },
}

impl QueryPlan {
    pub fn new() -> Self {
        Self { steps: Vec::new() }
    }

    pub fn add_step(&mut self, step: PlanStep) {
        self.steps.push(step);
    }
}

impl Default for QueryPlan {
    fn default() -> Self {
        Self::new()
    }
}
