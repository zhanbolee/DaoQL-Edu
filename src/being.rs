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
//! Being（实体）— 世界上存在的一切事物
//!
//! 教学说明：
//! - Being 是 DaoQL 的核心概念，代表一切可被标识的事物
//! - 由三部分组成：BeingCore（固定字段）+ BeingExt（动态字段）+ embeddings（向量）
//! - 写入时拆分为 NodeRecord（mmap 定长）和外部存储（ext/embedding）
//! - 这种"固定+动态"的设计兼顾了性能（固定字段内联）和灵活性（动态字段无 Schema 约束）

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::DaoQLError;
use crate::id::BeingId;

/// 内存中的完整 Being 表示
///
/// 这是用户操作的数据结构，从 NodeRecord + ext + embedding 组装而来。
/// 写入时拆分为多个部分分别存储。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Being {
    /// 固定核心字段
    pub core: BeingCore,
    /// 动态扩展属性（可选）
    pub ext: Option<BeingExt>,
    /// 嵌入向量（字段名 → 向量）
    pub embeddings: HashMap<String, Vec<f32>>,
}

impl Being {
    /// 创建新 Being（自动生成 BeingId）
    pub fn new(name: impl Into<String>, def: impl Into<String>) -> Self {
        Self {
            core: BeingCore::new(name, def),
            ext: None,
            embeddings: HashMap::new(),
        }
    }

    /// 从 BeingId 创建（用于读取后重建）
    pub fn with_id(id: BeingId) -> Self {
        Self {
            core: BeingCore::with_id(id),
            ext: None,
            embeddings: HashMap::new(),
        }
    }

    /// 从 NodeRecord 重建 Being
    pub fn from_node(node: &crate::graph::record::NodeRecord) -> Result<Self, DaoQLError> {
        Ok(Self {
            core: BeingCore {
                id: node.id,
                def: node.get_def_name(),
                status: node.status,
                name: node.get_name(),
                code: node.get_code(),
                description: node.get_description(),
                weight: node.weight,
                priority: node.priority,
                category: node.get_category(),
                created_at: node.created_at,
                updated_at: node.updated_at,
                tx_begin: node.tx_begin,
                tx_end: node.tx_end,
            },
            ext: None,
            embeddings: HashMap::new(),
        })
    }

    /// 添加动态字段
    pub fn with_attr(mut self, key: impl Into<String>, value: Value) -> Self {
        self.ext.get_or_insert_with(|| BeingExt {
            being_id: self.core.id,
            timestamp: self.core.updated_at,
            dynamic_attrs: HashMap::new(),
        });
        if let Some(ref mut ext) = self.ext {
            ext.dynamic_attrs.insert(key.into(), value);
        }
        self
    }

    /// 添加嵌入向量
    pub fn with_embedding(mut self, field: impl Into<String>, vector: Vec<f32>) -> Self {
        self.embeddings.insert(field.into(), vector);
        self
    }

    /// 获取动态字段值
    pub fn attr(&self, key: &str) -> Option<&Value> {
        self.ext.as_ref()?.dynamic_attrs.get(key)
    }

    /// 获取嵌入向量
    pub fn embedding(&self, field: &str) -> Option<&Vec<f32>> {
        self.embeddings.get(field)
    }

    /// 更新状态
    pub fn set_status(&mut self, status: u8) {
        self.core.status = status;
        self.core.updated_at = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
    }

    /// 验证 Being 完整性
    pub fn validate(&self) -> Result<(), DaoQLError> {
        if self.core.name.is_empty() {
            return Err(DaoQLError::ConstraintViolation(
                "Being name 不能为空".to_string(),
            ));
        }
        if self.core.def.is_empty() {
            return Err(DaoQLError::ConstraintViolation(
                "Being def 不能为空".to_string(),
            ));
        }
        // 验证向量维度一致性
        for (field, vec) in &self.embeddings {
            if vec.is_empty() {
                return Err(DaoQLError::ConstraintViolation(format!(
                    "嵌入向量 {field} 不能为空"
                )));
            }
        }
        Ok(())
    }
}

/// Being 固定核心字段
///
/// 这些字段内联存储在 NodeRecord（1536 bytes）中，实现 O(1) 随机访问。
/// 教学版包含约 20 个常用字段，实际生产系统可能有更多。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BeingCore {
    /// 全局唯一标识符
    pub id: BeingId,
    /// 类型定义名（如 "Order"）
    pub def: String,
    /// 状态码（0 = 正常，1 = 停用，等等）
    pub status: u8,
    /// 显示名称
    pub name: String,
    /// 业务编码
    pub code: String,
    /// 描述
    pub description: String,
    /// 权重（用于排序/优先级）
    pub weight: f64,
    /// 优先级
    pub priority: i32,
    /// 分类
    pub category: String,
    /// 创建时间（纳秒时间戳）
    pub created_at: i64,
    /// 更新时间（纳秒时间戳）
    pub updated_at: i64,
    /// MVCC 开始事务号
    pub tx_begin: u64,
    /// MVCC 结束事务号（u64::MAX = 活跃）
    pub tx_end: u64,
}

impl BeingCore {
    /// 创建新 BeingCore
    pub fn new(name: impl Into<String>, def: impl Into<String>) -> Self {
        let now = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
        Self {
            id: BeingId::new(),
            def: def.into(),
            status: 0,
            name: name.into(),
            code: String::new(),
            description: String::new(),
            weight: 0.0,
            priority: 0,
            category: String::new(),
            created_at: now,
            updated_at: now,
            tx_begin: 0,
            tx_end: u64::MAX,
        }
    }

    /// 从 BeingId 创建（用于读取后重建）
    pub fn with_id(id: BeingId) -> Self {
        let now = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
        Self {
            id,
            def: String::new(),
            status: 0,
            name: String::new(),
            code: String::new(),
            description: String::new(),
            weight: 0.0,
            priority: 0,
            category: String::new(),
            created_at: now,
            updated_at: now,
            tx_begin: 0,
            tx_end: u64::MAX,
        }
    }

    /// 判断在当前事务下是否可见
    ///
    /// MVCC 可见性规则（Read Committed）：
    /// - 已提交：tx_begin ≤ 当前事务号 且 (tx_end > 当前事务号 或 tx_end = u64::MAX)
    /// - 未提交：tx_begin 在当前活跃事务集合中
    pub fn is_visible(&self, tx_id: u64, active_txs: &[u64]) -> bool {
        // 当前事务自己创建的，总是可见
        if self.tx_begin == tx_id {
            return true;
        }
        // 创建者已提交（tx_begin 不在活跃集合中）
        let creator_committed = !active_txs.contains(&self.tx_begin);
        // 当前版本已结束（被更新/删除）
        let is_ended = self.tx_end != u64::MAX;
        // 结束者已提交
        let ender_committed = is_ended && !active_txs.contains(&self.tx_end);

        // 可见性：创建者已提交，且（未结束 或 结束者已提交）
        creator_committed && (!is_ended || ender_committed)
    }
}

/// Being 动态扩展属性
///
/// 无 Schema 约束的键值对存储。
/// 存储在外部文件（非 mmap 定长区域），通过 NodeRecord.ext_offset 引用。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BeingExt {
    /// 关联的 BeingId
    pub being_id: BeingId,
    /// 扩展属性时间戳
    pub timestamp: i64,
    /// 动态属性映射（键 → JSON 值）
    pub dynamic_attrs: HashMap<String, Value>,
}

impl BeingExt {
    /// 创建空扩展
    pub fn new(being_id: BeingId) -> Self {
        Self {
            being_id,
            timestamp: chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0),
            dynamic_attrs: HashMap::new(),
        }
    }

    /// 设置动态字段
    pub fn set(&mut self, key: impl Into<String>, value: Value) {
        self.dynamic_attrs.insert(key.into(), value);
        self.timestamp = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
    }

    /// 获取动态字段
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.dynamic_attrs.get(key)
    }

    /// 序列化为 JSON
    pub fn to_bytes(&self) -> Result<Vec<u8>, DaoQLError> {
        Ok(serde_json::to_vec(self)?)
    }

    /// 从 JSON 反序列化
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, DaoQLError> {
        Ok(serde_json::from_slice(bytes)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_being_creation() {
        let b = Being::new("Test Being", "TestDef");
        assert_eq!(b.core.name, "Test Being");
        assert_eq!(b.core.def, "TestDef");
        assert_eq!(b.core.status, 0);
        assert!(b.ext.is_none());
        assert!(b.embeddings.is_empty());
    }

    #[test]
    fn test_being_with_attr() {
        let b = Being::new("Product", "ProductDef")
            .with_attr("price", serde_json::json!(99.99))
            .with_attr("count", serde_json::json!(100));

        assert_eq!(b.attr("price"), Some(&serde_json::json!(99.99)));
        assert_eq!(b.attr("count"), Some(&serde_json::json!(100)));
        assert_eq!(b.attr("missing"), None);
    }

    #[test]
    fn test_being_with_embedding() {
        let emb = vec![0.1_f32, 0.2, 0.3];
        let b = Being::new("Doc", "DocDef").with_embedding("text_emb", emb.clone());
        assert_eq!(b.embedding("text_emb"), Some(&emb));
        assert_eq!(b.embedding("missing"), None);
    }

    #[test]
    fn test_being_validate() {
        let b = Being::new("", "Def");
        assert!(b.validate().is_err());

        let b = Being::new("Name", "");
        assert!(b.validate().is_err());

        let b = Being::new("Name", "Def");
        assert!(b.validate().is_ok());
    }

    #[test]
    fn test_being_core_visibility() {
        let mut core = Being::new("Test", "Def").core;
        core.tx_begin = 10;
        core.tx_end = u64::MAX;

        // 事务 20 读取：创建者 10 已提交（不在活跃集），未结束 → 可见
        assert!(core.is_visible(20, &[]));

        // 事务 20 读取：创建者 10 仍在活跃 → 不可见
        assert!(!core.is_visible(20, &[10]));

        // 事务 20 读取：已结束（tx_end=15），结束者已提交 → 可见（旧版本）
        core.tx_end = 15;
        assert!(core.is_visible(20, &[]));

        // 事务 20 读取：已结束，结束者未提交 → 不可见
        assert!(!core.is_visible(20, &[15]));

        // 自己创建的事务总是可见
        assert!(core.is_visible(10, &[]));
    }

    #[test]
    fn test_being_ext_roundtrip() {
        let id = BeingId::new();
        let mut ext = BeingExt::new(id);
        ext.set("color", serde_json::json!("red"));
        ext.set("size", serde_json::json!(42));

        let bytes = ext.to_bytes().unwrap();
        let ext2 = BeingExt::from_bytes(&bytes).unwrap();
        assert_eq!(ext.dynamic_attrs, ext2.dynamic_attrs);
    }
}
