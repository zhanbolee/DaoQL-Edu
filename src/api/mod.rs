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
//! Fluent API — Rust chainable call interface
//!
//! Educational Notes:
//! - provide type-safe chain API
//! - buildQuery、Executewrite、call DSL

pub mod dsl_api;
pub mod query_builder;
pub mod write_builder;

pub use dsl_api::DslApi;
pub use query_builder::QueryBuilder;
pub use write_builder::WriteBuilder;
