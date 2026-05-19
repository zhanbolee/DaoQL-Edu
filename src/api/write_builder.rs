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

use crate::being::Being;
use crate::def::DefRegistry;
use crate::error::DaoQLError;
use crate::graph::store::GraphStore;
use crate::id::{BeingId, NodeOffset};
use crate::index::uuid_index::UuidIndex;
use crate::relation::Relation;

/// Write builder
pub struct WriteBuilder<'a> {
    graph: &'a RefCell<GraphStore>,
    def_registry: &'a RefCell<DefRegistry>,
    uuid_index: &'a RefCell<UuidIndex>,
}

impl<'a> WriteBuilder<'a> {
    pub fn new(
        graph: &'a RefCell<GraphStore>,
        def_registry: &'a RefCell<DefRegistry>,
        uuid_index: &'a RefCell<UuidIndex>,
    ) -> Self {
        Self { graph, def_registry, uuid_index }
    }

    /// Create Being，writeGraph storage，return (BeingId, NodeOffset)
    pub fn create_being(&self, being: Being) -> Result<(BeingId, NodeOffset), DaoQLError> {
        let mut def_registry = self.def_registry.borrow_mut();
        let def = crate::def::Def::new(&being.core.def);
        let def_type_code = def_registry.register(def);
        drop(def_registry);
        let mut graph = self.graph.borrow_mut();
        let offset = graph.create_node(&being.core, def_type_code)?;
        Ok((being.core.id, offset))
    }

    /// establishRelation，writeGraph storage
    pub fn create_relation(&self, relation: Relation) -> Result<(), DaoQLError> {
        let mut graph = self.graph.borrow_mut();
        graph.create_edge(&relation)?;
        Ok(())
    }

    /// Delete Being (soft delete: mark status = 0)
    pub fn delete_being(&self, id: BeingId) -> Result<(), DaoQLError> {
        let uuid_index = self.uuid_index.borrow();
        let offset = uuid_index.get(id)?
            .ok_or(DaoQLError::NotFound(id))?;
        drop(uuid_index);
        let mut graph = self.graph.borrow_mut();
        let node = graph.read_node_mut(offset)?;
        node.status = 0;
        Ok(())
    }
}
