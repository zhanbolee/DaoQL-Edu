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
//! WAL (Write-Ahead Log)
//!
//! Educational Notes:
//! - WAL is the core mechanism for guaranteeing transaction durability
//! - flow: transaction writes WAL first → then modifies memory data → finally returns success
//! - Crash recovery: replay WAL at restart, redo already committed transactions
//! - dual-buffer group commit: Buffer A (foreground append) ↔ Buffer B (background fsync)

pub mod record;
pub mod recovery;
pub mod writer;

pub use record::{Op, TransactionPayload, WalRecord};
pub use recovery::WalRecovery;
pub use writer::WalWriter;
