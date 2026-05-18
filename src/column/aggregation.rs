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
//! SIMD 聚合实现
//!
//! 教学说明：
//! - SIMD（Single Instruction Multiple Data）：一次操作多个数据
//! - f64x4：一次处理 4 个 f64，理论加速 4x
//! - 实际加速约 2-3x（受内存带宽限制）
//! - 剩余不足 4 的尾部用标量处理
//! - 使用 `wide` crate（稳定可用，API 简洁）

use wide::f64x4;

use crate::error::DaoQLError;

/// 聚合操作类型
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AggregateOp {
    Count,
    Sum,
    Avg,
    Min,
    Max,
    Median,
    Stddev,
    Variance,
}

/// 聚合结果
#[derive(Debug, Clone, PartialEq)]
pub struct AggregateResult {
    pub op: AggregateOp,
    pub value: f64,
    pub count: usize,
}

/// SIMD Sum（f64）
///
/// 算法：
/// 1. 将数据分块为 f64x4（4 个 f64）
/// 2. 每块用 SIMD 累加
/// 3. 最后将 f64x4 的 4 个元素标量相加
/// 4. 处理不足 4 的尾部
pub fn simd_sum(values: &[f64]) -> f64 {
    let chunks = values.chunks_exact(4);
    let remainder = chunks.remainder();

    let mut sum_vec = f64x4::ZERO;
    for chunk in chunks {
        let v = f64x4::from(chunk);
        sum_vec += v;
    }

    let arr = sum_vec.as_array_ref();
    let mut result = arr[0] + arr[1] + arr[2] + arr[3];

    for &v in remainder {
        result += v;
    }

    result
}

/// SIMD 点积（f32）
pub fn simd_dot(a: &[f32], b: &[f32]) -> f32 {
    assert_eq!(a.len(), b.len());
    let min_len = a.len().min(b.len());
    let a = &a[..min_len];
    let b = &b[..min_len];

    let mut sum: f32 = 0.0;
    for i in 0..min_len {
        sum += a[i] * b[i];
    }
    sum
}

/// 标量聚合
pub fn aggregate(values: &[f64], op: AggregateOp) -> Result<AggregateResult, DaoQLError> {
    if values.is_empty() {
        return Ok(AggregateResult {
            op,
            value: 0.0,
            count: 0,
        });
    }

    let count = values.len();
    let value = match op {
        AggregateOp::Count => count as f64,
        AggregateOp::Sum => values.iter().sum(),
        AggregateOp::Avg => values.iter().sum::<f64>() / count as f64,
        AggregateOp::Min => values.iter().fold(f64::INFINITY, |a, &b| a.min(b)),
        AggregateOp::Max => values.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b)),
        AggregateOp::Median => {
            let mut sorted = values.to_vec();
            sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
            if count % 2 == 1 {
                sorted[count / 2]
            } else {
                (sorted[count / 2 - 1] + sorted[count / 2]) / 2.0
            }
        }
        AggregateOp::Stddev | AggregateOp::Variance => {
            let avg = values.iter().sum::<f64>() / count as f64;
            let variance = values.iter().map(|v| (v - avg).powi(2)).sum::<f64>() / count as f64;
            if op == AggregateOp::Variance {
                variance
            } else {
                variance.sqrt()
            }
        }
    };

    Ok(AggregateResult { op, value, count })
}

/// SIMD 与标量一致性验证（测试用）
pub fn verify_simd_consistency(values: &[f64]) -> bool {
    let simd = simd_sum(values);
    let scalar: f64 = values.iter().sum();
    (simd - scalar).abs() < 1e-10
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simd_sum_basic() {
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
        assert_eq!(simd_sum(&values), 36.0);
    }

    #[test]
    fn test_simd_sum_with_remainder() {
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        assert_eq!(simd_sum(&values), 15.0);
    }

    #[test]
    fn test_simd_sum_consistency() {
        let values: Vec<f64> = (0..1000).map(|i| i as f64 * 0.5).collect();
        assert!(verify_simd_consistency(&values));
    }

    #[test]
    fn test_aggregate_ops() {
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];

        assert_eq!(aggregate(&values, AggregateOp::Count).unwrap().value, 5.0);
        assert_eq!(aggregate(&values, AggregateOp::Sum).unwrap().value, 15.0);
        assert_eq!(aggregate(&values, AggregateOp::Avg).unwrap().value, 3.0);
        assert_eq!(aggregate(&values, AggregateOp::Min).unwrap().value, 1.0);
        assert_eq!(aggregate(&values, AggregateOp::Max).unwrap().value, 5.0);
        assert_eq!(aggregate(&values, AggregateOp::Median).unwrap().value, 3.0);
    }

    #[test]
    fn test_aggregate_stddev() {
        let values = vec![2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0];
        let result = aggregate(&values, AggregateOp::Stddev).unwrap();
        // 标准差 ≈ 2.0
        assert!((result.value - 2.0).abs() < 0.1);
    }

    #[test]
    fn test_simd_dot() {
        let a = vec![1.0_f32, 2.0, 3.0];
        let b = vec![4.0_f32, 5.0, 6.0];
        assert_eq!(simd_dot(&a, &b), 32.0);
    }
}
