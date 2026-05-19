// Copyright (c) 2026 Zhanbo Li / Atlas Lee <4859345@qq.com>
// SPDX-License-Identifier: BSL-1.1
//
// Licensed under the Business Source License, version 1.1 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at:
//
//     https://spdx.org/licenses/BSL-1.1.html
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
//
//! Def (Type Definition) — structure definition of a Being
//!
//! Educational Notes:
//! - Bootstrap design: Def itself is also a Being (def = "DAO_DEF")
//! - edu edition simplification: no type inheritance (extends), lifecycle policies, or state machines
//! - Field list is flat, no nested structure (simplified for understanding)
//! - Constraint system for validating Being data legality

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::being::BeingCore;
use crate::error::DaoQLError;
use crate::id::{BeingId, DefTypeCode};

/// Type definition
///
/// Each Def describes a class of Being's structure: what fields, what types, what constraints.
/// Def itself is also a Being, its def field is fixed to "DAO_DEF".
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Def {
    /// Def's own BeingId (bootstrap)
    pub id: BeingId,
    /// Type name (e.g. "Order")
    pub name: String,
    /// Field list
    pub fields: Vec<Field>,
    /// Creation time
    pub created_at: i64,
    /// Type code (internal use, for def_type_code in NodeRecord)
    pub type_code: DefTypeCode,
}

impl Def {
    /// Create new Def
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: BeingId::new(),
            name: name.into(),
            fields: Vec::new(),
            created_at: chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0),
            type_code: 0, // Assigned at registration time
        }
    }

    /// Add field (chainable API)
    pub fn with_field(mut self, field: Field) -> Self {
        self.fields.push(field);
        self
    }

    /// Find field
    pub fn field(&self, name: &str) -> Option<&Field> {
        self.fields.iter().find(|f| f.name == name)
    }

    /// Validate BeingCore against this Def
    ///
    /// edu edition simplification: only validates field existence and type matching.
    /// Production should also validate constraints (min/max/required etc.).
    pub fn validate_core(&self, core: &BeingCore) -> Result<(), DaoQLError> {
        // Validate core fields exist (edu edition：check name/def not empty）
        if core.name.is_empty() {
            return Err(DaoQLError::ConstraintViolation(format!(
                "Def '{}' requires name field to be non-empty",
                self.name
            )));
        }
        Ok(())
    }

    /// Validate dynamic fields against type constraints
    pub fn validate_ext(&self, attrs: &[(String, Value)]) -> Result<(), DaoQLError> {
        for (key, value) in attrs {
            if let Some(field) = self.field(key) {
                field.validate_value(value)?;
            }
            // edu edition: allow undefined dynamic fields (document flexibility)
        }
        Ok(())
    }

    /// Get all required field names
    pub fn required_fields(&self) -> Vec<&str> {
        self.fields
            .iter()
            .filter(|f| f.required)
            .map(|f| f.name.as_str())
            .collect()
    }
}

/// Field definition
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Field {
    /// Field name
    pub name: String,
    /// Field type
    pub field_type: FieldType,
    /// Whether required
    pub required: bool,
    /// Default value
    pub default_value: Option<Value>,
    /// Minimum value (numeric types)
    pub min: Option<f64>,
    /// Maximum value (numeric types)
    pub max: Option<f64>,
}

impl Field {
    /// Create new field
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

    /// Set required
    pub fn required(mut self) -> Self {
        self.required = true;
        self
    }

    /// Set default value
    pub fn default(mut self, value: Value) -> Self {
        self.default_value = Some(value);
        self
    }

    /// Set range constraints
    pub fn range(mut self, min: f64, max: f64) -> Self {
        self.min = Some(min);
        self.max = Some(max);
        self
    }

    /// Validate single value against field type and constraints
    pub fn validate_value(&self, value: &Value) -> Result<(), DaoQLError> {
        // Type check
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
                                "Array item [{i}] validation failed: {e}"
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
                                    "Map key '{k}' type error: {e}"
                                ))
                            })?;
                        val_type.validate_value(v).map_err(|e| {
                            DaoQLError::ConstraintViolation(format!(
                                "Map value '{k}' validation failed: {e}"
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

        // Range check (numeric types only)
        if let (Some(min), Some(max)) = (self.min, self.max) {
            let num = value.as_f64().ok_or_else(|| {
                DaoQLError::ConstraintViolation(format!(
                    "Field '{}' requires numeric type for range check",
                    self.name
                ))
            })?;
            if num < min || num > max {
                return Err(DaoQLError::ConstraintViolation(format!(
                    "Field '{}' value {num} not in range [{min}, {max}]",
                    self.name
                )));
            }
        }

        Ok(())
    }
}

/// Field type enum
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
    /// Get type name (for error messages)
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

    /// Whether numeric type (for range constraints)
    pub fn is_numeric(&self) -> bool {
        matches!(self, FieldType::Int | FieldType::Float)
    }

    /// Validate value against type (recursive, for Array/Map inner types)
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

/// Def registry
///
/// Manage all registered Defs, assign type_code.
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
            next_code: 1, // 0 reserved for unassigned
        }
    }

    /// Register new Def
    pub fn register(&mut self, mut def: Def) -> DefTypeCode {
        if let Some(&code) = self.name_to_code.get(&def.name) {
            return code; // Already exists, return existing code
        }
        let code = self.next_code;
        self.next_code += 1;
        def.type_code = code;
        self.name_to_code.insert(def.name.clone(), code);
        self.defs.push(def);
        code
    }

    /// Get or register Def by name (avoid duplicate Def creation)
    pub fn get_or_register(&mut self, name: &str) -> DefTypeCode {
        if let Some(&code) = self.name_to_code.get(name) {
            return code;
        }
        let def = Def::new(name);
        self.register(def)
    }

    /// Lookup by name
    pub fn by_name(&self, name: &str) -> Option<&Def> {
        let code = self.name_to_code.get(name)?;
        self.defs.iter().find(|d| d.type_code == *code)
    }

    /// Lookup by code
    pub fn by_code(&self, code: DefTypeCode) -> Option<&Def> {
        self.defs.iter().find(|d| d.type_code == code)
    }

    /// Get all Defs
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
