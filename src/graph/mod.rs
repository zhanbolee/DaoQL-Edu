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
//! 图引擎（Graph Engine）
//!
//! 教学说明：
//! - 基于 mmap 定长记录（NodeRecord/EdgeRecord）
//! - 免索引邻接：边通过 next_out/next_in 指针内联链接
//! - 支持 BFS、DFS、PageRank 等图算法
//! - PerBeingLock 实现细粒度并发控制

pub mod lock;
pub mod record;
pub mod store;
pub mod traversal;

pub use lock::{BeingLock, LockManager};
pub use record::{EdgeRecord, NodeRecord};
pub use store::GraphStore;
pub use traversal::{bfs, dfs, pagerank, shortest_path, EdgeFilter};
