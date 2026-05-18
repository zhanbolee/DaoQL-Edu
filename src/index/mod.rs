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
//! 索引层
//!
//! 教学说明：
//! - UUID → NodeOffset：强一致索引，使用 redb（纯 Rust B+Tree）
//! - Time Range：二级索引，支持 created_at/updated_at 范围查询
//! - Skip Index：列引擎内联，用于 Granule 级剪枝
//!
//! redb 特性：
//! - 纯 Rust 实现，无需外部进程
//! - 支持 ACID 事务
//! - 教学版数据量小，性能足够

pub mod time_index;
pub mod uuid_index;

pub use time_index::TimeIndex;
pub use uuid_index::UuidIndex;
