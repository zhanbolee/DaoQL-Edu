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
//! Granule — column storage data block (64KB)
//!
//! Educational Notes:
//! - Granule is column store basic I/O unit
//! - fixed 64KB size, matches OS page size，reduce I/O amplification
//! - each Granule contains a continuous column value + Skip Index metadata

use crate::column::skip_index::GranuleMeta;

/// Data Granule
pub struct Granule {
    pub meta: GranuleMeta,
    pub data: Vec<u8>,
}

impl Granule {
    pub fn new(offset: u64) -> Self {
        Self {
            meta: GranuleMeta::new(offset, 0),
            data: Vec::with_capacity(64 * 1024),
        }
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}
