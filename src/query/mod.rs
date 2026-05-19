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
//! Query Engine
//!
//! Educational Notes:
//! - Query route: determine which data engine
//! - Queryplan：simpleplangenerate
//! - Execute：iterationmode，lazy evaluation

pub mod executor;
pub mod planner;
pub mod router;

pub use executor::{QueryEngine, QueryResult};
pub use planner::QueryPlan;
pub use router::{EngineRoute, QueryRouter};
