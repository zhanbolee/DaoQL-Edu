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
