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

use serde::{Deserialize, Serialize};

use crate::id::{BeingId, RelationTypeCode};

/// RelationType definition
///
/// e.g.: HAS_PARENT=1, CREATED_BY=2, BELONGS_TO=3
/// Typeencoding is compact u16，convenient for storage in EdgeRecord in。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RelationType {
    /// Relation type code
    pub code: RelationTypeCode,
    /// Relation name
    pub name: String,
    /// Whether directed
    pub directed: bool,
}

impl RelationType {
    /// Create new relation type
    pub fn new(code: RelationTypeCode, name: impl Into<String>, directed: bool) -> Self {
        Self {
            code,
            name: name.into(),
            directed,
        }
    }
}

/// Relation instance
///
/// A specific edge, connecting from_id → to_id.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Relation {
    /// Source entity
    pub from_id: BeingId,
    /// Target entity
    pub to_id: BeingId,
    /// Relation type code
    pub relation_type: RelationTypeCode,
    /// Whether directed
    pub directed: bool,
    /// Creation time
    pub created_at: i64,
    /// Name（optional）
    pub name: String,
    /// Weight（optional）
    pub weight: f64,
}

impl Relation {
    /// CreatenewRelation
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

    /// Set name（chain）
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// Setweight（chain）
    pub fn with_weight(mut self, weight: f64) -> Self {
        self.weight = weight;
        self
    }

    /// reverse direction（for undirected edges or reverse queries）
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

    /// Serialize to postcard
    pub fn to_bytes(&self) -> Result<Vec<u8>, crate::error::DaoQLError> {
        Ok(postcard::to_allocvec(self)?)
    }

    /// From postcard deserialize
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, crate::error::DaoQLError> {
        Ok(postcard::from_bytes(bytes)?)
    }
}

/// Relationtype registry
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

    /// Registernewtype
    pub fn register(&mut self, rt: RelationType) {
        if self.name_to_code.contains_key(&rt.name) {
            return;
        }
        self.name_to_code.insert(rt.name.clone(), rt.code);
        self.types.push(rt);
    }

    /// Lookup by name
    pub fn by_name(&self, name: &str) -> Option<&RelationType> {
        let code = self.name_to_code.get(name)?;
        self.types.iter().find(|t| t.code == *code)
    }

    /// Lookup by code
    pub fn by_code(&self, code: RelationTypeCode) -> Option<&RelationType> {
        self.types.iter().find(|t| t.code == code)
    }

    /// Getalltype
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
