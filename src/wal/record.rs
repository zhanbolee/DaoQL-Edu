// Copyright 2026 Zhanbo Li / Atlas Lee <zhanbo.lee@hotmail.com>
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use serde::{Deserialize, Serialize};

use crate::being::Being;
use crate::error::DaoQLError;
use crate::id::BeingId;
use crate::relation::Relation;
use crate::transaction::TxId;

/// WAL magic number
pub const WAL_MAGIC: [u8; 4] = *b"WAL\x01";

/// WAL recordheadsize
pub const WAL_HEADER_SIZE: usize = 16; // magic(4) + len(4) + seq(8)

/// WAL record
///
/// binary format：
/// ```ignore
/// [magic: 4 bytes]     = b"WAL\x01"
/// [payload_len: 4 bytes] — little-endian u32
/// [seq: 8 bytes]         — little-endian u64
/// [payload: N bytes]     — postcard serialized TransactionPayload
/// [crc32: 4 bytes]       — payload CRC32 checksum
/// ```ignore
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WalRecord {
    pub magic: [u8; 4],
    pub payload_len: u32,
    pub seq: u64,
    pub payload: Vec<u8>,
    pub crc32: u32,
}

impl WalRecord {
    /// Createnewrecord
    pub fn new(seq: u64, payload: Vec<u8>) -> Self {
        let crc32 = crc32fast::hash(&payload);
        Self {
            magic: WAL_MAGIC,
            payload_len: payload.len() as u32,
            seq,
            payload,
            crc32,
        }
    }

    /// Serialize to bytes
    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(WAL_HEADER_SIZE + self.payload.len() + 4);
        buf.extend_from_slice(&self.magic);
        buf.extend_from_slice(&self.payload_len.to_le_bytes());
        buf.extend_from_slice(&self.seq.to_le_bytes());
        buf.extend_from_slice(&self.payload);
        buf.extend_from_slice(&self.crc32.to_le_bytes());
        buf
    }

    /// Secondary byte stream parse (may contain multiple records)
    ///
    /// Return: (parsed record, remaining byte count)
    pub fn parse_one(data: &[u8]) -> Result<(Option<Self>, usize), DaoQLError> {
        if data.len() < WAL_HEADER_SIZE + 4 {
            return Ok((None, 0));
        }

        // checkmagic number
        if data[0..4] != WAL_MAGIC {
            return Err(DaoQLError::Wal("WAL magic number mismatch".to_string()));
        }

        let payload_len = u32::from_le_bytes([data[4], data[5], data[6], data[7]]) as usize;
        let seq = u64::from_le_bytes([
            data[8], data[9], data[10], data[11],
            data[12], data[13], data[14], data[15],
        ]);
        let total_len = WAL_HEADER_SIZE + payload_len + 4;

        if data.len() < total_len {
            return Ok((None, 0)); // data incomplete
        }

        let payload = data[WAL_HEADER_SIZE..WAL_HEADER_SIZE + payload_len].to_vec();
        let stored_crc = u32::from_le_bytes([
            data[WAL_HEADER_SIZE + payload_len],
            data[WAL_HEADER_SIZE + payload_len + 1],
            data[WAL_HEADER_SIZE + payload_len + 2],
            data[WAL_HEADER_SIZE + payload_len + 3],
        ]);

        // CRC checksum
        let computed_crc = crc32fast::hash(&payload);
        if computed_crc != stored_crc {
            return Err(DaoQLError::Wal(format!(
                "CRC check failed: seq={seq}, expect={stored_crc:08x}, actual={computed_crc:08x}"
            )));
        }

        let record = Self {
            magic: WAL_MAGIC,
            payload_len: payload_len as u32,
            seq,
            payload,
            crc32: stored_crc,
        };

        Ok((Some(record), total_len))
    }

    /// Validate CRC
    pub fn verify_crc(&self) -> bool {
        crc32fast::hash(&self.payload) == self.crc32
    }
}

/// Transaction operation enum
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[allow(clippy::large_enum_variant)]
pub enum Op {
    /// Create Being
    CreateBeing { being: Being },
    /// Update Being
    UpdateBeing {
        id: BeingId,
        /// FieldUpdatelist
        updates: Vec<FieldUpdate>,
    },
    /// Create relation
    CreateRelation { relation: Relation },
    /// Delete Being (soft delete)
    DeleteBeing { id: BeingId },
}

/// FieldUpdate
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FieldUpdate {
    pub field_name: String,
    pub new_value: serde_json::Value,
}

/// Transactionpayload
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TransactionPayload {
    pub tx_id: TxId,
    pub ops: Vec<Op>,
}

impl TransactionPayload {
    pub fn new(tx_id: TxId) -> Self {
        Self {
            tx_id,
            ops: Vec::new(),
        }
    }

    pub fn add_op(mut self, op: Op) -> Self {
        self.ops.push(op);
        self
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, DaoQLError> {
        Ok(postcard::to_allocvec(self)?)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, DaoQLError> {
        Ok(postcard::from_bytes(bytes)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wal_record_serialize_roundtrip() {
        let payload = TransactionPayload::new(42)
            .add_op(Op::CreateBeing {
                being: crate::being::Being::new("Test", "Def"),
            });
        let payload_bytes = payload.to_bytes().unwrap();

        let record = WalRecord::new(1, payload_bytes);
        let bytes = record.serialize();

        let (parsed, consumed) = WalRecord::parse_one(&bytes).unwrap();
        assert!(parsed.is_some());
        assert_eq!(consumed, bytes.len());

        let parsed = parsed.unwrap();
        assert_eq!(parsed.seq, 1);
        assert!(parsed.verify_crc());
    }

    #[test]
    fn test_wal_record_crc_failure() {
        let payload = b"test payload".to_vec();
        let mut record = WalRecord::new(1, payload);
        record.crc32 = 0xDEADBEEF; // tamper CRC

        let bytes = record.serialize();
        let result = WalRecord::parse_one(&bytes);
        assert!(result.is_err());
    }

    #[test]
    fn test_wal_record_partial_data() {
        let data = vec![0u8; 10]; // insufficient for a record header
        let (parsed, _) = WalRecord::parse_one(&data).unwrap();
        assert!(parsed.is_none());
    }

    #[test]
    fn test_transaction_payload_roundtrip() {
        let id = BeingId::new();
        let payload = TransactionPayload::new(10)
            .add_op(Op::CreateRelation {
                relation: crate::relation::Relation::new(id, id, 1, true),
            });
        let bytes = payload.to_bytes().unwrap();
        let payload2 = TransactionPayload::from_bytes(&bytes).unwrap();
        assert_eq!(payload.tx_id, payload2.tx_id);
        assert_eq!(payload.ops.len(), payload2.ops.len());
    }
}
