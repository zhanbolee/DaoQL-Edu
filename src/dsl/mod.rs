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
//! DSL (Declarative Query Language)
//!
//! Educational Notes:
//! - GraphQL-like syntax
//! - support：query / mutation / analyze / define
//! - recursive descentParse

pub mod ast;
pub mod executor;
pub mod lexer;
pub mod parser;

pub use ast::{AstNode, DslQuery};
pub use executor::DslExecutor;
pub use lexer::{Lexer, Token};
pub use parser::Parser;
