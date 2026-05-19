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
