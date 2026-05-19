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
//! SIMD Distance Computation
//!
//! Educational Notes:
//! - vectorsearchcoreisdistancecompute（Cosine / L2 / Dot）
//! - SIMD accelerate：process at once 4-8 Dimension，theoretical speedup 4-8x
//! - use `wide` crate  f32x8 / f64x4
//! - remaining tail processed with scalar

use serde::{Deserialize, Serialize};
use wide::f32x8;

/// Distance metric
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DistanceMetric {
    Cosine,
    L2,
    Dot,
}

/// Cosine similarity = 1 - cosine_distance
/// Cosine distance = 1 - (a·b / (|a| * |b|))
pub fn cosine_distance(a: &[f32], b: &[f32]) -> f32 {
    assert_eq!(a.len(), b.len());
    let dot = dot_product(a, b);
    let norm_a = dot_product(a, a).sqrt();
    let norm_b = dot_product(b, b).sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        return 1.0;
    }
    1.0 - dot / (norm_a * norm_b)
}

/// L2 Euclidean distance = sqrt(Σ(a_i - b_i)²)
pub fn l2_distance(a: &[f32], b: &[f32]) -> f32 {
    assert_eq!(a.len(), b.len());
    let chunks = a.chunks_exact(8);
    let b_chunks = b.chunks_exact(8);
    let remainder = chunks.remainder();
    let b_remainder = b_chunks.remainder();

    let mut sum_vec = f32x8::ZERO;
    for (a_chunk, b_chunk) in chunks.zip(b_chunks) {
        let av = f32x8::from(a_chunk);
        let bv = f32x8::from(b_chunk);
        let diff = av - bv;
        sum_vec += diff * diff;
    }

    let arr = sum_vec.as_array_ref();
    let mut sum = arr[0] + arr[1] + arr[2] + arr[3] + arr[4] + arr[5] + arr[6] + arr[7];

    for i in 0..remainder.len() {
        let diff = remainder[i] - b_remainder[i];
        sum += diff * diff;
    }

    sum.sqrt()
}

/// Dot product = Σ(a_i * b_i)
pub fn dot_product(a: &[f32], b: &[f32]) -> f32 {
    assert_eq!(a.len(), b.len());

    #[cfg(target_arch = "aarch64")]
    unsafe {
        dot_product_neon(a, b)
    }
    #[cfg(not(target_arch = "aarch64"))]
    {
        let chunks = a.chunks_exact(8);
        let b_chunks = b.chunks_exact(8);
        let remainder = chunks.remainder();
        let b_remainder = b_chunks.remainder();

        let mut sum_vec = f32x8::ZERO;
        for (a_chunk, b_chunk) in chunks.zip(b_chunks) {
            let av = f32x8::from(a_chunk);
            let bv = f32x8::from(b_chunk);
            sum_vec += av * bv;
        }

        let arr = sum_vec.as_array_ref();
        let mut sum = arr[0] + arr[1] + arr[2] + arr[3] + arr[4] + arr[5] + arr[6] + arr[7];

        for i in 0..remainder.len() {
            sum += remainder[i] * b_remainder[i];
        }

        sum
    }
}

/// aarch64 NEON accelerated dot product (Apple M-series specific optimization path)
#[cfg(target_arch = "aarch64")]
#[inline]
unsafe fn dot_product_neon(a: &[f32], b: &[f32]) -> f32 {
    use std::arch::aarch64::*;
    let n = a.len();
    let mut sum1 = vdupq_n_f32(0.0);
    let mut sum2 = vdupq_n_f32(0.0);
    let mut i = 0;
    while i + 8 <= n {
        let a1 = vld1q_f32(a.as_ptr().add(i));
        let a2 = vld1q_f32(a.as_ptr().add(i + 4));
        let b1 = vld1q_f32(b.as_ptr().add(i));
        let b2 = vld1q_f32(b.as_ptr().add(i + 4));
        sum1 = vfmaq_f32(sum1, a1, b1);
        sum2 = vfmaq_f32(sum2, a2, b2);
        i += 8;
    }
    let sum = vaddq_f32(sum1, sum2);
    let arr: [f32; 4] = std::mem::transmute(sum);
    let mut total = arr[0] + arr[1] + arr[2] + arr[3];
    while i < n {
        total += a[i] * b[i];
        i += 1;
    }
    total
}

/// L2 normalize vector (in-place modify)
pub fn l2_normalize(vec: &mut [f32]) {
    let sq: f32 = vec.iter().map(|x| x * x).sum();
    let norm = sq.sqrt();
    if norm > 1e-10 {
        for x in vec.iter_mut() {
            *x /= norm;
        }
    }
}

/// By metric compute distance
pub fn compute_distance(a: &[f32], b: &[f32], metric: DistanceMetric) -> f32 {
    match metric {
        DistanceMetric::Cosine => cosine_distance(a, b),
        DistanceMetric::L2 => l2_distance(a, b),
        DistanceMetric::Dot => -dot_product(a, b), // Dot similarity negated to distance
    }
}

/// Pre-normalization Cosine fast path: assume a, b already L2-normalized, distance = 1.0 - dot(a, b)
pub fn cosine_fast_path(a: &[f32], b: &[f32]) -> f32 {
    1.0 - dot_product(a, b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosine_distance() {
        let a = vec![1.0_f32, 0.0, 0.0];
        let b = vec![0.0_f32, 1.0, 0.0];
        // orthogonal vector, cosine = 0, distance = 1
        assert!((cosine_distance(&a, &b) - 1.0).abs() < 0.001);

        let c = vec![1.0_f32, 0.0, 0.0];
        assert!(cosine_distance(&a, &c).abs() < 0.001);
    }

    #[test]
    fn test_l2_distance() {
        let a = vec![0.0_f32, 0.0];
        let b = vec![3.0_f32, 4.0];
        assert!((l2_distance(&a, &b) - 5.0).abs() < 0.001);
    }

    #[test]
    fn test_dot_product() {
        let a = vec![1.0_f32, 2.0, 3.0];
        let b = vec![4.0_f32, 5.0, 6.0];
        assert!((dot_product(&a, &b) - 32.0).abs() < 0.001);
    }

    #[test]
    fn test_simd_vs_scalar_consistency() {
        let a: Vec<f32> = (0..100).map(|i| (i as f32) * 0.1).collect();
        let b: Vec<f32> = (0..100).map(|i| (i as f32) * 0.05).collect();

        let simd_l2 = l2_distance(&a, &b);
        let scalar_l2: f32 = a.iter().zip(b.iter()).map(|(x, y)| (x - y).powi(2)).sum::<f32>().sqrt();
        assert!((simd_l2 - scalar_l2).abs() < 1e-3);

        let simd_dot = dot_product(&a, &b);
        let scalar_dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        assert!((simd_dot - scalar_dot).abs() < 1e-3);
    }
}
