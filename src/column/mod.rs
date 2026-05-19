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
//! Column Engine
//!
//! Educational Notes:
//! - dual-layer architecture：RawLayer (row store) + ProjectedLayer (column store)
//! - RawLayer: append-only, suitable for document flexibility
//! - ProjectedLayer: hot field columnar materialization，suitable for aggregate analysis
//! - Skip Index: Granule level min/max, query pruning
//! - SIMD Aggregate: f64x4 vectorized execution

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
