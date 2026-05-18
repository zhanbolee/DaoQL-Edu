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
//! 向量引擎（Vector Engine）
//!
//! 教学说明：
//! - HNSW：多层图结构，近似最近邻搜索
//! - SIMD 距离：Cosine / L2 / Dot 的向量化加速
//! - 量化：标量量化(SQ) / 二进制量化(BQ)，减少内存占用
//! - 图先验：利用 Relation 邻接节点作为 HNSW 插入种子
//! - RCU 热更新：读无锁，写复制

pub mod distance;
pub mod graph_prior;
pub mod hnsw;
pub mod quantization;

pub use distance::{cosine_distance, l2_distance, dot_product, DistanceMetric};
pub use hnsw::{HnswIndex, SearchResult};
pub use quantization::{BinaryQuantization, ScalarQuantization};
