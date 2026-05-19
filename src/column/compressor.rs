// Copyright (c) 2026 Zhanbo Li / Atlas Lee <4859345@qq.com>
// SPDX-License-Identifier: AGPL-3.0-or-later
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published
// by the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

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
