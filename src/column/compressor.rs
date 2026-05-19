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
//! Compressor — lz4_flex wrapper
//!
//! Educational Notes:
//! - lz4 is an ultra-fast compression algorithm，compression ratio moderate，speed extremely fast
//! - suitable for real-time data engine：compress/decompress latency < 1ms/MB
//! - edu edition uses pure Rust lz4_flex，no C dependencies

use crate::error::DaoQLError;

/// Compress data
pub fn compress(data: &[u8]) -> Vec<u8> {
    lz4_flex::compress(data)
}

/// Decompress data
pub fn decompress(data: &[u8], uncompressed_size: usize) -> Result<Vec<u8>, DaoQLError> {
    lz4_flex::decompress(data, uncompressed_size)
        .map_err(|e| DaoQLError::InvalidState(format!("decompress failed: {e:?}")))
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
