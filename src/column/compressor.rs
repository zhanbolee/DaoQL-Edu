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
//! 压缩器 — lz4_flex 封装
//!
//! 教学说明：
//! - lz4 是一种超快速的压缩算法，压缩比适中，速度极快
//! - 适合实时数据引擎：压缩/解压延迟 < 1ms/MB
//! - 教学版使用纯 Rust 实现 lz4_flex，无需 C 依赖

use crate::error::DaoQLError;

/// 压缩数据
pub fn compress(data: &[u8]) -> Vec<u8> {
    lz4_flex::compress(data)
}

/// 解压数据
pub fn decompress(data: &[u8], uncompressed_size: usize) -> Result<Vec<u8>, DaoQLError> {
    lz4_flex::decompress(data, uncompressed_size)
        .map_err(|e| DaoQLError::InvalidState(format!("解压失败: {e:?}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compress_roundtrip() {
        let data = b"Hello, this is some test data for compression! ".repeat(100);
        let compressed = compress(&data);
        assert!(compressed.len() < data.len());

        let decompressed = decompress(&compressed, data.len()).unwrap();
        assert_eq!(decompressed, data);
    }
}
