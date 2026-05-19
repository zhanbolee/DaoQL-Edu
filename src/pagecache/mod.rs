// Copyright (c) 2026 Zhanbo Li / Atlas Lee <4859345@qq.com>
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
//! Page Cache (PageCache)
//!
//! Educational Notes:
//! - Clock Sweep algorithm：classic OS cache eviction algorithm
//! - 16 partition：reduce lock contention
//! - Each cache page has a "reference bit" (ref bit)

pub mod clock_sweep;

pub use clock_sweep::{CachePage, CacheShard, PageCache};
