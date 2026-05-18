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
//! WAL（Write-Ahead Log）— 预写日志
//!
//! 教学说明：
//! - WAL 是保证事务持久性的核心机制
//! - 流程：事务先写 WAL → 再修改内存数据 → 最后返回成功
//! - 崩溃恢复：重启时回放 WAL，重做已提交事务
//! - 双缓冲组提交：Buffer A（前台追加）↔ Buffer B（后台 fsync）

pub mod record;
pub mod recovery;
pub mod writer;

pub use record::{Op, TransactionPayload, WalRecord};
pub use recovery::WalRecovery;
pub use writer::WalWriter;
