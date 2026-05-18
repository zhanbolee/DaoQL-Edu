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
//! 列引擎（Column Engine）
//!
//! 教学说明：
//! - 双层架构：RawLayer（行存）+ ProjectedLayer（列存）
//! - RawLayer：append-only，适合文档灵活性
//! - ProjectedLayer：热点字段列式物化，适合聚合分析
//! - Skip Index：Granule 级 min/max，查询剪枝
//! - SIMD 聚合：f64x4 向量化执行

pub mod aggregation;
pub mod compressor;
pub mod granule;
pub mod projected_layer;
pub mod raw_layer;
pub mod skip_index;

pub use aggregation::{simd_sum, AggregateOp, AggregateResult};
pub use projected_layer::{ProjectedColumn, ProjectedLayer};
pub use raw_layer::{RawLayer, RawRecord};
pub use skip_index::{GranuleMeta, SkipIndex};
