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
//! Def（类型定义）— Being 的结构定义
//!
//! 教学说明：
//! - 自举设计：Def 本身也是一种 Being（def = "DAO_DEF"）
//! - 教学版简化：不支持类型继承（extends）、生命周期策略、状态机
//! - 字段列表是 flat 的，无嵌套结构（简化理解）
//! - 约束系统用于验证 Being 数据合法性

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::being::BeingCore;
use crate::error::DaoQLError;
use crate::id::{BeingId, DefTypeCode};

/// 类型定义
///
/// 每个 Def 描述一类 Being 的结构：有哪些字段、什么类型、什么约束。
/// Def 自身也是 Being，其 def 字段固定为 "DAO_DEF"。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Def {
    /// Def 自身的 BeingId（自举）
    pub id: BeingId,
    /// 类型名（如 "Order"）
    pub name: String,
    /// 字段列表
    pub fields: Vec<Field>,
    /// 创建时间
    pub created_at: i64,
    /// 类型编码（内部使用，用于 NodeRecord 中的 def_type_code）
    pub type_code: DefTypeCode,
}

impl Def {
    /// 创建新 Def
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: BeingId::new(),
            name: name.into(),
            fields: Vec::new(),
            created_at: chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0),
            type_code: 0, // 由注册时分配
        }
    }

    /// 添加字段（链式 API）
    pub fn with_field(mut self, field: Field) -> Self {
        self.fields.push(field);
        self
    }

    /// 查找字段
    pub fn field(&self, name: &str) -> Option<&Field> {
        self.fields.iter().find(|f| f.name == name)
    }

    /// 验证 BeingCore 是否符合本 Def
    ///
    /// 教学版简化：仅验证字段存在性和类型匹配。
    /// 生产版还应验证约束（min/max/required 等）。
    pub fn validate_core(&self, core: &BeingCore) -> Result<(), DaoQLError> {
        // 验证核心字段存在（教学版：检查 name/def 非空）
        if core.name.is_empty() {
            return Err(DaoQLError::ConstraintViolation(format!(
                "Def '{}' 要求 name 字段非空",
                self.name
            )));
        }
        Ok(())
    }

    /// 验证动态字段是否符合类型约束
    pub fn validate_ext(&self, attrs: &[(String, Value)]) -> Result<(), DaoQLError> {
        for (key, value) in attrs {
            if let Some(field) = self.field(key) {
                field.validate_value(value)?;
            }
            // 教学版：允许未定义的动态字段（文档灵活性）
        }
        Ok(())
    }

    /// 获取所有 required 字段名
    pub fn required_fields(&self) -> Vec<&str> {
        self.fields
            .iter()
            .filter(|f| f.required)
            .map(|f| f.name.as_str())
            .collect()
    }
}

/// 字段定义
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Field {
    /// 字段名
    pub name: String,
    /// 字段类型
    pub field_type: FieldType,
    /// 是否必填
    pub required: bool,
    /// 默认值
    pub default_value: Option<Value>,
    /// 最小值（数值类型）
    pub min: Option<f64>,
    /// 最大值（数值类型）
    pub max: Option<f64>,
}

impl Field {
    /// 创建新字段
    pub fn new(name: impl Into<String>, field_type: FieldType) -> Self {
        Self {
            name: name.into(),
            field_type,
            required: false,
            default_value: None,
            min: None,
            max: None,
        }
    }

    /// 设置必填
    pub fn required(mut self) -> Self {
        self.required = true;
        self
    }

    /// 设置默认值
    pub fn default(mut self, value: Value) -> Self {
        self.default_value = Some(value);
        self
    }

    /// 设置范围约束
    pub fn range(mut self, min: f64, max: f64) -> Self {
        self.min = Some(min);
        self.max = Some(max);
        self
    }

    /// 验证单个值是否符合字段类型和约束
    pub fn validate_value(&self, value: &Value) -> Result<(), DaoQLError> {
        // 类型检查
        match &self.field_type {
            FieldType::String => {
                if !value.is_string() {
                    return Err(DaoQLError::TypeMismatch {
                        expected: "String".to_string(),
                        actual: value.to_string(),
                    });
                }
            }
            FieldType::Int => {
                if !value.is_i64() && !value.is_u64() {
                    return Err(DaoQLError::TypeMismatch {
                        expected: "Int".to_string(),
                        actual: value.to_string(),
                    });
                }
            }
            FieldType::Float => {
                if !value.is_number() {
                    return Err(DaoQLError::TypeMismatch {
                        expected: "Float".to_string(),
                        actual: value.to_string(),
                    });
                }
            }
            FieldType::Bool => {
                if !value.is_boolean() {
                    return Err(DaoQLError::TypeMismatch {
                        expected: "Bool".to_string(),
                        actual: value.to_string(),
                    });
                }
            }
            FieldType::Array(inner) => {
                if let Some(arr) = value.as_array() {
                    for (i, item) in arr.iter().enumerate() {
                        inner.validate_value(item).map_err(|e| {
                            DaoQLError::ConstraintViolation(format!(
                                "数组项 [{i}] 验证失败: {e}"
                            ))
                        })?;
                    }
                } else {
                    return Err(DaoQLError::TypeMismatch {
                        expected: "Array".to_string(),
                        actual: value.to_string(),
                    });
                }
            }
            FieldType::Map(key_type, val_type) => {
                if let Some(obj) = value.as_object() {
                    for (k, v) in obj.iter() {
                        key_type
                            .validate_value(&Value::String(k.clone()))
                            .map_err(|e| {
                                DaoQLError::ConstraintViolation(format!(
                                    "Map 键 '{k}' 类型错误: {e}"
                                ))
                            })?;
                        val_type.validate_value(v).map_err(|e| {
                            DaoQLError::ConstraintViolation(format!(
                                "Map 值 '{k}' 验证失败: {e}"
                            ))
                        })?;
                    }
                } else {
                    return Err(DaoQLError::TypeMismatch {
                        expected: "Map".to_string(),
                        actual: value.to_string(),
                    });
                }
            }
        }

        // 范围检查（仅数值类型）
        if let (Some(min), Some(max)) = (self.min, self.max) {
            let num = value.as_f64().ok_or_else(|| {
                DaoQLError::ConstraintViolation(format!(
                    "字段 '{}' 需要数值类型才能进行范围检查",
                    self.name
                ))
            })?;
            if num < min || num > max {
                return Err(DaoQLError::ConstraintViolation(format!(
                    "字段 '{}' 的值 {num} 不在范围 [{min}, {max}] 内",
                    self.name
                )));
            }
        }

        Ok(())
    }
}

/// 字段类型枚举
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FieldType {
    String,
    Int,
    Float,
    Bool,
    Array(Box<FieldType>),
    Map(Box<FieldType>, Box<FieldType>),
}

impl FieldType {
    /// 获取类型名（用于错误信息）
    pub fn type_name(&self) -> &'static str {
        match self {
            FieldType::String => "String",
            FieldType::Int => "Int",
            FieldType::Float => "Float",
            FieldType::Bool => "Bool",
            FieldType::Array(_) => "Array",
            FieldType::Map(_, _) => "Map",
        }
    }

    /// 是否数值类型（可用于范围约束）
    pub fn is_numeric(&self) -> bool {
        matches!(self, FieldType::Int | FieldType::Float)
    }

    /// 验证值是否符合类型（递归，用于 Array/Map 内部类型）
    pub fn validate_value(&self, value: &serde_json::Value) -> Result<(), DaoQLError> {
        match self {
            FieldType::String => {
                if !value.is_string() {
                    return Err(DaoQLError::TypeMismatch {
                        expected: "String".to_string(),
                        actual: value.to_string(),
                    });
                }
            }
            FieldType::Int => {
                if !value.is_i64() && !value.is_u64() {
                    return Err(DaoQLError::TypeMismatch {
                        expected: "Int".to_string(),
                        actual: value.to_string(),
                    });
                }
            }
            FieldType::Float => {
                if !value.is_number() {
                    return Err(DaoQLError::TypeMismatch {
                        expected: "Float".to_string(),
                        actual: value.to_string(),
                    });
                }
            }
            FieldType::Bool => {
                if !value.is_boolean() {
                    return Err(DaoQLError::TypeMismatch {
                        expected: "Bool".to_string(),
                        actual: value.to_string(),
                    });
                }
            }
            FieldType::Array(inner) => {
                if let Some(arr) = value.as_array() {
                    for item in arr {
                        inner.validate_value(item)?;
                    }
                } else {
                    return Err(DaoQLError::TypeMismatch {
                        expected: "Array".to_string(),
                        actual: value.to_string(),
                    });
                }
            }
            FieldType::Map(key_type, val_type) => {
                if let Some(obj) = value.as_object() {
                    for (k, v) in obj.iter() {
                        key_type.validate_value(&serde_json::Value::String(k.clone()))?;
                        val_type.validate_value(v)?;
                    }
                } else {
                    return Err(DaoQLError::TypeMismatch {
                        expected: "Map".to_string(),
                        actual: value.to_string(),
                    });
                }
            }
        }
        Ok(())
    }
}

/// Def 注册表
///
/// 管理所有已注册的 Def，分配 type_code。
pub struct DefRegistry {
    defs: Vec<Def>,
    name_to_code: std::collections::HashMap<String, DefTypeCode>,
    next_code: DefTypeCode,
}

impl DefRegistry {
    pub fn new() -> Self {
        Self {
            defs: Vec::new(),
            name_to_code: std::collections::HashMap::new(),
            next_code: 1, // 0 保留为未分配
        }
    }

    /// 注册新 Def
    pub fn register(&mut self, mut def: Def) -> DefTypeCode {
        if let Some(&code) = self.name_to_code.get(&def.name) {
            return code; // 已存在，返回已有编码
        }
        let code = self.next_code;
        self.next_code += 1;
        def.type_code = code;
        self.name_to_code.insert(def.name.clone(), code);
        self.defs.push(def);
        code
    }

    /// 按名称获取或注册定义（避免重复创建 Def 对象）
    pub fn get_or_register(&mut self, name: &str) -> DefTypeCode {
        if let Some(&code) = self.name_to_code.get(name) {
            return code;
        }
        let def = Def::new(name);
        self.register(def)
    }

    /// 按名称查找
    pub fn by_name(&self, name: &str) -> Option<&Def> {
        let code = self.name_to_code.get(name)?;
        self.defs.iter().find(|d| d.type_code == *code)
    }

    /// 按编码查找
    pub fn by_code(&self, code: DefTypeCode) -> Option<&Def> {
        self.defs.iter().find(|d| d.type_code == code)
    }

    /// 获取所有 Def
    pub fn all(&self) -> &[Def] {
        &self.defs
    }
}

impl Default for DefRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_def_creation() {
        let def = Def::new("Order")
            .with_field(Field::new("amount", FieldType::Float).required())
            .with_field(Field::new("status", FieldType::Int).default(serde_json::json!(0)));

        assert_eq!(def.name, "Order");
        assert_eq!(def.fields.len(), 2);
        assert!(def.field("amount").unwrap().required);
        assert_eq!(
            def.field("status").unwrap().default_value,
            Some(serde_json::json!(0))
        );
    }

    #[test]
    fn test_field_validation() {
        let f = Field::new("age", FieldType::Int).range(0.0, 150.0);

        assert!(f.validate_value(&serde_json::json!(25)).is_ok());
        assert!(f.validate_value(&serde_json::json!(-1)).is_err());
        assert!(f.validate_value(&serde_json::json!(200)).is_err());
        assert!(f.validate_value(&serde_json::json!("hello")).is_err());
    }

    #[test]
    fn test_array_field_validation() {
        let inner = FieldType::Int;
        let f = Field::new("tags", FieldType::Array(Box::new(inner)));

        assert!(f
            .validate_value(&serde_json::json!([1, 2, 3]))
            .is_ok());
        assert!(f
            .validate_value(&serde_json::json!([1, "two", 3]))
            .is_err());
    }

    #[test]
    fn test_def_registry() {
        let mut reg = DefRegistry::new();
        let def1 = Def::new("Order");
        let def2 = Def::new("Customer");

        let code1 = reg.register(def1);
        let code2 = reg.register(def2);

        assert_ne!(code1, code2);
        assert!(reg.by_name("Order").is_some());
        assert!(reg.by_code(code1).is_some());
        assert_eq!(reg.all().len(), 2);
    }

    #[test]
    fn test_def_registry_duplicate() {
        let mut reg = DefRegistry::new();
        let code1 = reg.register(Def::new("Order"));
        let code2 = reg.register(Def::new("Order"));
        assert_eq!(code1, code2);
        assert_eq!(reg.all().len(), 1);
    }
}
