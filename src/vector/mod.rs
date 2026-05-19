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
//! Vector Engine（Vector Engine）
//!
//! Educational Notes:
//! - HNSW: multi-layer graph structure, approximate nearest neighbor search
//! - SIMD distance: Cosine / L2 / Dot vectorized acceleration
//! - Quantization: Scalar quantization (SQ) / binary quantization (BQ), reduce memory usage
//! - Graph prior: use relation adjacent nodes as HNSW insert seeds
//! - RCU hot update: read lock-free, write copy

pub mod distance;
pub mod graph_prior;
pub mod hnsw;
pub mod quantization;

pub use distance::{cosine_distance, l2_distance, dot_product, DistanceMetric};
pub use hnsw::{HnswIndex, SearchResult};
pub use quantization::{BinaryQuantization, ScalarQuantization};
