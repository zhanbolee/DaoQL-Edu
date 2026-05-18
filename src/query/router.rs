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
//! 查询路由器
//!
//! 教学说明：
//! - 根据查询特征判定走哪条引擎
//! - 路由规则：点查→图、聚合→列、向量→向量、混合→多引擎


/// 查询目标引擎
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EngineRoute {
    Graph,
    Column,
    Vector,
    Multi,
}

/// 查询路由器
pub struct QueryRouter;

impl QueryRouter {
    pub fn new() -> Self {
        Self
    }

    /// 路由查询
    pub fn route(&self, query_type: &str) -> EngineRoute {
        match query_type {
            "point" | "traversal" => EngineRoute::Graph,
            "scan" | "aggregate" => EngineRoute::Column,
            "similar" => EngineRoute::Vector,
            _ => EngineRoute::Multi,
        }
    }
}

impl Default for QueryRouter {
    fn default() -> Self {
        Self::new()
    }
}
