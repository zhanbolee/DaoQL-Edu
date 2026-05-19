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
