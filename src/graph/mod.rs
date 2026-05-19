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
//! Graph Engine（Graph Engine）
//!
//! Educational Notes:
//! - Based on mmap fixed-length records (NodeRecord/EdgeRecord)
//! - Index-free adjacency: edges inline-linked via next_out/next_in pointers
//! - support BFS, DFS, PageRank and other graph algorithms
//! - PerBeingLock implement fine-grained concurrency control

pub mod lock;
pub mod record;
pub mod store;
pub mod traversal;

pub use lock::{BeingLock, LockManager};
pub use record::{EdgeRecord, NodeRecord};
pub use store::GraphStore;
pub use traversal::{bfs, dfs, pagerank, shortest_path, EdgeFilter};
