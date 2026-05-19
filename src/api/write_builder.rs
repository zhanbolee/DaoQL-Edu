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
