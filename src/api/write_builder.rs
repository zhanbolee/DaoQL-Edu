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
//! 写入构建器
//!
//! 教学说明：
//! - 链式 API 构建写入操作
//! - 支持 Being 创建、更新、删除
//! - 关系建立

use std::cell::RefCell;

use crate::being::Being;
use crate::def::DefRegistry;
use crate::error::DaoQLError;
use crate::graph::store::GraphStore;
use crate::id::{BeingId, NodeOffset};
use crate::index::uuid_index::UuidIndex;
use crate::relation::Relation;

/// 写入构建器
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

    /// 创建 Being，写入图存储，返回 (BeingId, NodeOffset)
    pub fn create_being(&self, being: Being) -> Result<(BeingId, NodeOffset), DaoQLError> {
        let mut def_registry = self.def_registry.borrow_mut();
        let def = crate::def::Def::new(&being.core.def);
        let def_type_code = def_registry.register(def);
        drop(def_registry);
        let mut graph = self.graph.borrow_mut();
        let offset = graph.create_node(&being.core, def_type_code)?;
        Ok((being.core.id, offset))
    }

    /// 建立关系，写入图存储
    pub fn create_relation(&self, relation: Relation) -> Result<(), DaoQLError> {
        let mut graph = self.graph.borrow_mut();
        graph.create_edge(&relation)?;
        Ok(())
    }

    /// 删除 Being（软删除：标记 status = 0）
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
