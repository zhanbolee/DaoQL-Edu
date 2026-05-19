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
//! Index Layer
//!
//! Educational Notes:
//! - UUID → NodeOffset: strongly consistent index, use redb (pure Rust B+Tree)
//! - Time Range: two-level index, support created_at/updated_at range query
//! - Skip Index: column engine inline, used for Granule level pruning
//!
//! redb features：
//! - pure Rust implementation, no external process
//! - support ACID transaction
//! - edu edition data volume small, performance sufficient

pub mod time_index;
pub mod uuid_index;

pub use time_index::TimeIndex;
pub use uuid_index::UuidIndex;
