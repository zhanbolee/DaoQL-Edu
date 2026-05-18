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
//! Relation（关系）— Being 之间的连接
//!
//! 教学说明：
//! - Relation 是有向/无向边，连接两个 Being
//! - 边本身也是定长记录（EdgeRecord, 256 bytes），存储在 mmap 中
//! - 通过 next_out/next_in 指针形成邻接链表，实现免索引邻接
//! - 教学版简化：不支持时态有效性、weight、metadata

use serde::{Deserialize, Serialize};

use crate::id::{BeingId, RelationTypeCode};

/// 关系类型定义
///
/// 如：HAS_PARENT=1, CREATED_BY=2, BELONGS_TO=3
/// 类型编码是紧凑的 u16，便于存储在 EdgeRecord 中。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RelationType {
    /// 关系类型编码
    pub code: RelationTypeCode,
    /// 关系名称
    pub name: String,
    /// 是否有向
    pub directed: bool,
}

impl RelationType {
    /// 创建新关系类型
    pub fn new(code: RelationTypeCode, name: impl Into<String>, directed: bool) -> Self {
        Self {
            code,
            name: name.into(),
            directed,
        }
    }
}

/// 关系实例
///
/// 一条具体的边，连接 from_id → to_id。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Relation {
    /// 源实体
    pub from_id: BeingId,
    /// 目标实体
    pub to_id: BeingId,
    /// 关系类型编码
    pub relation_type: RelationTypeCode,
    /// 是否有向
    pub directed: bool,
    /// 创建时间
    pub created_at: i64,
    /// 名称（可选）
    pub name: String,
    /// 权重（可选）
    pub weight: f64,
}

impl Relation {
    /// 创建新关系
    pub fn new(
        from_id: BeingId,
        to_id: BeingId,
        relation_type: RelationTypeCode,
        directed: bool,
    ) -> Self {
        Self {
            from_id,
            to_id,
            relation_type,
            directed,
            created_at: chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0),
            name: String::new(),
            weight: 1.0,
        }
    }

    /// 设置名称（链式）
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// 设置权重（链式）
    pub fn with_weight(mut self, weight: f64) -> Self {
        self.weight = weight;
        self
    }

    /// 反转方向（用于无向边或反向查询）
    pub fn reversed(&self) -> Self {
        Self {
            from_id: self.to_id,
            to_id: self.from_id,
            relation_type: self.relation_type,
            directed: self.directed,
            created_at: self.created_at,
            name: self.name.clone(),
            weight: self.weight,
        }
    }

    /// 序列化为 postcard
    pub fn to_bytes(&self) -> Result<Vec<u8>, crate::error::DaoQLError> {
        Ok(postcard::to_allocvec(self)?)
    }

    /// 从 postcard 反序列化
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, crate::error::DaoQLError> {
        Ok(postcard::from_bytes(bytes)?)
    }
}

/// 关系类型注册表
pub struct RelationTypeRegistry {
    types: Vec<RelationType>,
    name_to_code: std::collections::HashMap<String, RelationTypeCode>,
}

impl RelationTypeRegistry {
    pub fn new() -> Self {
        Self {
            types: Vec::new(),
            name_to_code: std::collections::HashMap::new(),
        }
    }

    /// 注册新类型
    pub fn register(&mut self, rt: RelationType) {
        if self.name_to_code.contains_key(&rt.name) {
            return;
        }
        self.name_to_code.insert(rt.name.clone(), rt.code);
        self.types.push(rt);
    }

    /// 按名称查找
    pub fn by_name(&self, name: &str) -> Option<&RelationType> {
        let code = self.name_to_code.get(name)?;
        self.types.iter().find(|t| t.code == *code)
    }

    /// 按编码查找
    pub fn by_code(&self, code: RelationTypeCode) -> Option<&RelationType> {
        self.types.iter().find(|t| t.code == code)
    }

    /// 获取所有类型
    pub fn all(&self) -> &[RelationType] {
        &self.types
    }
}

impl Default for RelationTypeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_relation_type() {
        let rt = RelationType::new(1, "HAS_PARENT", true);
        assert_eq!(rt.code, 1);
        assert_eq!(rt.name, "HAS_PARENT");
        assert!(rt.directed);
    }

    #[test]
    fn test_relation_creation() {
        let id1 = BeingId::new();
        let id2 = BeingId::new();
        let rel = Relation::new(id1, id2, 1, true)
            .with_name("parent_of")
            .with_weight(2.5);

        assert_eq!(rel.from_id, id1);
        assert_eq!(rel.to_id, id2);
        assert_eq!(rel.relation_type, 1);
        assert_eq!(rel.name, "parent_of");
        assert_eq!(rel.weight, 2.5);
    }

    #[test]
    fn test_relation_reversed() {
        let id1 = BeingId::new();
        let id2 = BeingId::new();
        let rel = Relation::new(id1, id2, 1, true).with_name("A_to_B");
        let rev = rel.reversed();

        assert_eq!(rev.from_id, id2);
        assert_eq!(rev.to_id, id1);
        assert_eq!(rev.name, "A_to_B");
    }

    #[test]
    fn test_relation_roundtrip() {
        let id1 = BeingId::new();
        let id2 = BeingId::new();
        let rel = Relation::new(id1, id2, 1, true);
        let bytes = rel.to_bytes().unwrap();
        let rel2 = Relation::from_bytes(&bytes).unwrap();
        assert_eq!(rel.from_id, rel2.from_id);
        assert_eq!(rel.to_id, rel2.to_id);
        assert_eq!(rel.relation_type, rel2.relation_type);
    }

    #[test]
    fn test_relation_registry() {
        let mut reg = RelationTypeRegistry::new();
        reg.register(RelationType::new(1, "HAS_PARENT", true));
        reg.register(RelationType::new(2, "CREATED_BY", true));

        assert!(reg.by_name("HAS_PARENT").is_some());
        assert!(reg.by_code(2).is_some());
        assert_eq!(reg.all().len(), 2);
    }
}
