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
//! Vector Quantization — reduces memory usage
//!
//! Educational Notes:
//! - Scalar quantization (SQ): will compress f32 to u8 (1/4 memory)
//! - Binary quantization (BQ): will convert f32 to 0/1 bit graph (1/32 memory)
//! - Quantization is lossy: precision for memory, suitable for large-scale vector retrieval

/// Scalar quantization：f32 → u8
///
/// willvectorvaluemapto [0, 255] range：
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

/// binaryQuantization：f32 → bit
///
/// positive value → 1, negative value/zero → 0
/// Use Hamming distance instead of float distance, compute extremely fast
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

    /// Hamming distance = different bit count
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

        // Quantization is lossy, but should be in reasonable range
        for (orig, deq) in vec.iter().zip(d.iter()) {
            assert!((orig - deq).abs() < 0.01);
        }
    }

    #[test]
    fn test_binary_quantization() {
        let vec = vec![-1.0, 0.5, -0.3, 0.8];
        let bits = BinaryQuantization::quantize(&vec);
        // Expected: 0, 1, 0, 1 → binary 1010 → 0x0A (little-endian)
        assert_eq!(bits[0], 0b00001010);
    }

    #[test]
    fn test_hamming_distance() {
        let a = vec![0b10101010u8];
        let b = vec![0b11110000u8];
        // 10101010 ^ 11110000 = 01011010 → bits 1,3,5,6 = 4 differences
        assert_eq!(BinaryQuantization::hamming_distance(&a, &b), 4);
    }
}
