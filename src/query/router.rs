// Copyright (c) 2026 Zhanbo Li / Atlas Lee <4859345@qq.com>
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
//! Query Router
//!
//! Educational Notes:
//! - determine which engine based on query features
//! - route rule: point query→graph, aggregate→column, vector→vector, mixed→multi-engine


/// Query target engine
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EngineRoute {
    Graph,
    Column,
    Vector,
    Multi,
}

/// Query router
pub struct QueryRouter;

impl QueryRouter {
    pub fn new() -> Self {
        Self
    }

    /// routeQuery
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
