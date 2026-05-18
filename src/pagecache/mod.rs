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
//! 页面缓存（PageCache）
//!
//! 教学说明：
//! - Clock Sweep 算法：经典操作系统缓存淘汰算法
//! - 16 分区：减少锁竞争
//! - 每个缓存页有一个 "引用位"（ref bit）

pub mod clock_sweep;

pub use clock_sweep::{CachePage, CacheShard, PageCache};
