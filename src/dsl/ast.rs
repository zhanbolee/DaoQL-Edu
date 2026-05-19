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

use crate::being::Being;
use crate::id::BeingId;

/// DSL Querytype
#[derive(Debug, Clone, PartialEq)]
pub enum DslQuery {
    /// Query
    Query {
        target: String,
        filter: Option<FilterExpr>,
        projections: Vec<String>,
        limit: Option<usize>,
        history: Option<HistoryExpr>,
        aggregate: Option<(String, String)>,
    },
    /// Mutation
    Mutation {
        op: MutationOp,
        target: String,
        input: Vec<(String, serde_json::Value)>,
    },
    /// Analyze
    Analyze {
        algorithm: String,
        target: String,
        limit: Option<usize>,
    },
    /// Type definition
    Define {
        name: String,
        fields: Vec<FieldDef>,
    },
    /// Vector similarity
    Similar {
        target: String,
        query_vector: Vec<f32>,
        k: usize,
    },
}

/// Filter expression
#[derive(Debug, Clone, PartialEq)]
pub enum FilterExpr {
    Eq { field: String, value: serde_json::Value },
    Gt { field: String, value: serde_json::Value },
    Lt { field: String, value: serde_json::Value },
    Gte { field: String, value: serde_json::Value },
    Lte { field: String, value: serde_json::Value },
    And(Box<FilterExpr>, Box<FilterExpr>),
    Or(Box<FilterExpr>, Box<FilterExpr>),
}

/// History expression
#[derive(Debug, Clone, PartialEq)]
pub enum HistoryExpr {
    All,
    Last(usize),
    At(i64),
}

/// Mutationoperation
#[derive(Debug, Clone, PartialEq)]
pub enum MutationOp {
    Create,
    Update,
    Delete,
}

/// Field definition
#[derive(Debug, Clone, PartialEq)]
pub struct FieldDef {
    pub name: String,
    pub field_type: String,
    pub required: bool,
    pub default: Option<serde_json::Value>,
}

/// AST Node
#[derive(Debug, Clone, PartialEq)]
pub enum AstNode {
    Query(DslQuery),
    Being(Being),
    BeingId(BeingId),
    Value(serde_json::Value),
}
