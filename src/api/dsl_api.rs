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
//! DSL API Entry Point
//!
//! Educational Notes:
//! - will parse and execute DSL string
//! - unified entry point：execute_dsl(dsl_str)

use std::cell::RefCell;
use std::collections::HashMap;

use crate::def::DefRegistry;
use crate::dsl::ast::DslQuery;
use crate::dsl::executor::{DslExecutor, DslResult};
use crate::dsl::parser::Parser;
use crate::error::DaoQLError;
use crate::graph::store::GraphStore;
use crate::column::ProjectedLayer;
use crate::index::uuid_index::UuidIndex;
use crate::vector::hnsw::HnswIndex;

/// DSL API
pub struct DslApi {
    executor: DslExecutor,
}

impl DslApi {
    pub fn new() -> Self {
        Self {
            executor: DslExecutor::new(),
        }
    }

    /// Execute DSL string
    pub fn execute(
        &self,
        dsl: &str,
        graph: &RefCell<GraphStore>,
        def_registry: &RefCell<DefRegistry>,
        vector_indices: &HashMap<String, HnswIndex>,
        uuid_index: &RefCell<UuidIndex>,
        column: &RefCell<ProjectedLayer>,
    ) -> Result<DslResult, DaoQLError> {
        let ast = self.parse(dsl)?;
        self.executor.execute(&ast, graph, def_registry, vector_indices, uuid_index, column)
    }

    /// Parse DSL (without execute)
    pub fn parse(&self, dsl: &str) -> Result<DslQuery, DaoQLError> {
        let mut parser = Parser::new(dsl)?;
        parser.parse()
    }
}

impl Default for DslApi {
    fn default() -> Self {
        Self::new()
    }
}
