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
//! DSL AST（抽象语法树）
//!
//! 教学说明：
//! - AST 是解析器和执行器之间的中间表示
//! - 每个节点对应一种 DSL 语法结构

use crate::being::Being;
use crate::id::BeingId;

/// DSL 查询类型
#[derive(Debug, Clone, PartialEq)]
pub enum DslQuery {
    /// 查询
    Query {
        target: String,
        filter: Option<FilterExpr>,
        projections: Vec<String>,
        limit: Option<usize>,
        history: Option<HistoryExpr>,
        aggregate: Option<(String, String)>,
    },
    /// 变更
    Mutation {
        op: MutationOp,
        target: String,
        input: Vec<(String, serde_json::Value)>,
    },
    /// 分析
    Analyze {
        algorithm: String,
        target: String,
        limit: Option<usize>,
    },
    /// 类型定义
    Define {
        name: String,
        fields: Vec<FieldDef>,
    },
    /// 向量相似
    Similar {
        target: String,
        query_vector: Vec<f32>,
        k: usize,
    },
}

/// 过滤表达式
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

/// 历史表达式
#[derive(Debug, Clone, PartialEq)]
pub enum HistoryExpr {
    All,
    Last(usize),
    At(i64),
}

/// 变更操作
#[derive(Debug, Clone, PartialEq)]
pub enum MutationOp {
    Create,
    Update,
    Delete,
}

/// 字段定义
#[derive(Debug, Clone, PartialEq)]
pub struct FieldDef {
    pub name: String,
    pub field_type: String,
    pub required: bool,
    pub default: Option<serde_json::Value>,
}

/// AST 节点
#[derive(Debug, Clone, PartialEq)]
pub enum AstNode {
    Query(DslQuery),
    Being(Being),
    BeingId(BeingId),
    Value(serde_json::Value),
}
