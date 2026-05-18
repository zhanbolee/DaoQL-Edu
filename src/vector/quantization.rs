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
//! 向量量化 — 减少内存占用
//!
//! 教学说明：
//! - 标量量化(SQ)：将 f32 压缩为 u8（1/4 内存）
//! - 二进制量化(BQ)：将 f32 转为 0/1 位图（1/32 内存）
//! - 量化有损：精度换内存，适合大规模向量检索

/// 标量量化：f32 → u8
///
/// 将向量值映射到 [0, 255] 范围：
/// quantized = clamp((value - min) / (max - min) * 255, 0, 255)
pub struct ScalarQuantization {
    pub min: f32,
    pub max: f32,
}

impl ScalarQuantization {
    pub fn new(min: f32, max: f32) -> Self {
        Self { min, max }
    }

    pub fn quantize(&self, vector: &[f32]) -> Vec<u8> {
        let range = self.max - self.min;
        if range == 0.0 {
            return vec![0; vector.len()];
        }
        vector
            .iter()
            .map(|&v| {
                let normalized = ((v - self.min) / range).clamp(0.0, 1.0);
                (normalized * 255.0) as u8
            })
            .collect()
    }

    pub fn dequantize(&self, quantized: &[u8]) -> Vec<f32> {
        let range = self.max - self.min;
        quantized
            .iter()
            .map(|&q| self.min + (q as f32 / 255.0) * range)
            .collect()
    }
}

/// 二进制量化：f32 → bit
///
/// 正值 → 1，负值/零 → 0
/// 使用 Hamming 距离替代浮点距离，计算极快
pub struct BinaryQuantization;

impl BinaryQuantization {
    pub fn quantize(vector: &[f32]) -> Vec<u8> {
        let bits_len = vector.len().div_ceil(8);
        let mut bits = vec![0u8; bits_len];
        for (i, &v) in vector.iter().enumerate() {
            if v > 0.0 {
                bits[i / 8] |= 1 << (i % 8);
            }
        }
        bits
    }

    /// Hamming 距离 = 不同位的数量
    pub fn hamming_distance(a: &[u8], b: &[u8]) -> u32 {
        a.iter()
            .zip(b.iter())
            .map(|(x, y)| (x ^ y).count_ones())
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scalar_quantization_roundtrip() {
        let sq = ScalarQuantization::new(-1.0, 1.0);
        let vec = vec![-0.5, 0.0, 0.5, 1.0];
        let q = sq.quantize(&vec);
        let d = sq.dequantize(&q);

        // 量化有损，但应在合理范围内
        for (orig, deq) in vec.iter().zip(d.iter()) {
            assert!((orig - deq).abs() < 0.01);
        }
    }

    #[test]
    fn test_binary_quantization() {
        let vec = vec![-1.0, 0.5, -0.3, 0.8];
        let bits = BinaryQuantization::quantize(&vec);
        // 预期: 0, 1, 0, 1 → 二进制 1010 → 0x0A（小端）
        assert_eq!(bits[0], 0b00001010);
    }

    #[test]
    fn test_hamming_distance() {
        let a = vec![0b10101010u8];
        let b = vec![0b11110000u8];
        // 10101010 ^ 11110000 = 01011010 → 位1,3,5,6 = 4 个不同
        assert_eq!(BinaryQuantization::hamming_distance(&a, &b), 4);
    }
}
