// Copyright 2026 Zhanbo Li / Atlas Lee <4859345@qq.com>
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

use crate::id::{BeingId, DefTypeCode, RelationTypeCode};

/// GraphNode record — fixed-length 1536 bytes
///
/// Memory layout：
/// ```ignore
/// [0..18]    id: BeingId (18 bytes)
/// [18..20]   def_type_code: u16
/// [20]       status: u8
/// [21..32]   _pad1
/// [32..40]   created_at: i64
/// [40..48]   updated_at: i64
/// [48..56]   tx_begin: u64
/// [56..64]   tx_end: u64
/// [64..72]   prev_version_offset: u64
/// [72..80]   next_version_offset: u64
/// [80..88]   first_out_edge_offset: u64
/// [88..96]   first_in_edge_offset: u64
/// [96..160]  name: [u8; 64]
/// [160..192] code: [u8; 32]
/// [192..320] description: [u8; 128]
/// [320..328] weight: f64
/// [328..332] priority: i32
/// [332..336] _pad2
/// [336..352] category: [u8; 16]
/// [352..360] ext_offset: u64
/// [360..368] embedding_offset: u64
/// [368..1536] reserved
/// ```ignore
#[repr(C)]
pub struct NodeRecord {
    // === Core metadata (90 bytes) ===
    pub id: BeingId,
    pub def_type_code: DefTypeCode,
    pub status: u8,
    _pad1: [u8; 11],
    pub created_at: i64,
    pub updated_at: i64,
    pub tx_begin: u64,
    pub tx_end: u64,

    // === Version chain pointer (16 bytes) ===
    pub prev_version_offset: u64,
    pub next_version_offset: u64,

    // === Relation pointer (16 bytes) ===
    pub first_out_edge_offset: u64,
    pub first_in_edge_offset: u64,

    // === Inline core fields (256 bytes) ===
    pub name: [u8; 64],
    pub code: [u8; 32],
    pub description: [u8; 128],
    pub weight: f64,
    pub priority: i32,
    _pad2: [u8; 4],
    pub category: [u8; 16],

    // === External storage pointer (16 bytes) ===
    pub ext_offset: u64,
    pub embedding_offset: u64,

    // === Type name (64 bytes) ===
    pub def_name: [u8; 64],

    // === Reserved space (1110 bytes) ===
    pub reserved: [u8; 1104],
}

impl NodeRecord {
    pub const SIZE: usize = 1536;

    /// Create empty record
    pub fn new(id: BeingId) -> Self {
        Self {
            id,
            def_type_code: 0,
            status: 0,
            _pad1: [0; 11],
            created_at: 0,
            updated_at: 0,
            tx_begin: 0,
            tx_end: u64::MAX,
            prev_version_offset: 0,
            next_version_offset: 0,
            first_out_edge_offset: u64::MAX,
            first_in_edge_offset: u64::MAX,
            name: [0; 64],
            code: [0; 32],
            description: [0; 128],
            weight: 0.0,
            priority: 0,
            _pad2: [0; 4],
            category: [0; 16],
            ext_offset: 0,
            embedding_offset: 0,
            def_name: [0; 64],
            reserved: [0; 1104],
        }
    }

    /// Settype name（UTF-8，truncate to 63 bytes + null）
    pub fn set_def_name(&mut self, name: &str) {
        let bytes = name.as_bytes();
        let len = bytes.len().min(63);
        self.def_name[..len].copy_from_slice(&bytes[..len]);
        self.def_name[len] = 0;
    }

    /// Gettype name（secondary null-terminated C string）
    pub fn get_def_name(&self) -> String {
        let len = self.def_name.iter().position(|&b| b == 0).unwrap_or(64);
        String::from_utf8_lossy(&self.def_name[..len]).to_string()
    }

    /// Set name（UTF-8，truncate to 63 bytes + null）
    pub fn set_name(&mut self, name: &str) {
        let bytes = name.as_bytes();
        let len = bytes.len().min(63);
        self.name[..len].copy_from_slice(&bytes[..len]);
        self.name[len] = 0;
    }

    /// Get name（secondary null-terminated C string）
    pub fn get_name(&self) -> String {
        let len = self.name.iter().position(|&b| b == 0).unwrap_or(64);
        String::from_utf8_lossy(&self.name[..len]).to_string()
    }

    /// Setencoding
    pub fn set_code(&mut self, code: &str) {
        let bytes = code.as_bytes();
        let len = bytes.len().min(31);
        self.code[..len].copy_from_slice(&bytes[..len]);
        self.code[len] = 0;
    }

    /// Getencoding
    pub fn get_code(&self) -> String {
        let len = self.code.iter().position(|&b| b == 0).unwrap_or(32);
        String::from_utf8_lossy(&self.code[..len]).to_string()
    }

    /// SetDescription
    pub fn set_description(&mut self, desc: &str) {
        let bytes = desc.as_bytes();
        let len = bytes.len().min(127);
        self.description[..len].copy_from_slice(&bytes[..len]);
        self.description[len] = 0;
    }

    /// GetDescription
    pub fn get_description(&self) -> String {
        let len = self.description.iter().position(|&b| b == 0).unwrap_or(128);
        String::from_utf8_lossy(&self.description[..len]).to_string()
    }

    /// SetCategory
    pub fn set_category(&mut self, cat: &str) {
        let bytes = cat.as_bytes();
        let len = bytes.len().min(15);
        self.category[..len].copy_from_slice(&bytes[..len]);
        self.category[len] = 0;
    }

    /// GetCategory
    pub fn get_category(&self) -> String {
        let len = self.category.iter().position(|&b| b == 0).unwrap_or(16);
        String::from_utf8_lossy(&self.category[..len]).to_string()
    }

    /// Is emptyrecord
    pub fn is_empty(&self) -> bool {
        self.id.is_null()
    }

    /// Whether is current active version
    pub fn is_active(&self) -> bool {
        self.tx_end == u64::MAX
    }
}

/// GraphEdge record — fixed-length 256 bytes
///
/// Memory layout：
/// ```ignore
/// [0..18]    from_id: BeingId
/// [18..36]   to_id: BeingId
/// [36..38]   relation_type: u16
/// [38]       directed: u8
/// [39]       _pad1
/// [40..48]   created_at: i64
/// [48..56]   tx_begin: u64
/// [56..64]   tx_end: u64
/// [64..72]   next_out_edge_offset: u64
/// [72..80]   next_in_edge_offset: u64
/// [80..112]  name: [u8; 32]
/// [112..120] weight: f64
/// [120..256] _pad2
/// ```ignore
#[repr(C)]
pub struct EdgeRecord {
    pub from_id: BeingId,
    pub to_id: BeingId,
    pub relation_type: RelationTypeCode,
    pub directed: u8,
    _pad1: [u8; 1],
    pub created_at: i64,
    pub tx_begin: u64,
    pub tx_end: u64,
    pub next_out_edge_offset: u64,
    pub next_in_edge_offset: u64,
    pub name: [u8; 32],
    pub weight: f64,
    _pad2: [u8; 136],
}

impl EdgeRecord {
    pub const SIZE: usize = 256;

    /// Create empty Edge record
    pub fn empty() -> Self {
        Self {
            from_id: BeingId::null(),
            to_id: BeingId::null(),
            relation_type: 0,
            directed: 0,
            _pad1: [0; 1],
            created_at: 0,
            tx_begin: 0,
            tx_end: u64::MAX,
            next_out_edge_offset: u64::MAX,
            next_in_edge_offset: u64::MAX,
            name: [0; 32],
            weight: 0.0,
            _pad2: [0; 136],
        }
    }

    /// Set name
    pub fn set_name(&mut self, name: &str) {
        let bytes = name.as_bytes();
        let len = bytes.len().min(31);
        self.name[..len].copy_from_slice(&bytes[..len]);
        self.name[len] = 0;
    }

    /// Get name
    pub fn get_name(&self) -> String {
        let len = self.name.iter().position(|&b| b == 0).unwrap_or(32);
        String::from_utf8_lossy(&self.name[..len]).to_string()
    }

    /// Is emptyedge
    pub fn is_empty(&self) -> bool {
        self.from_id.is_null()
    }
}

// compile-time size assertion
#[allow(dead_code)]
const fn assert_sizes() {
    assert!(std::mem::size_of::<NodeRecord>() == 1536);
    assert!(std::mem::size_of::<EdgeRecord>() == 256);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_record_size() {
        assert_eq!(std::mem::size_of::<NodeRecord>(), 1536);
    }

    #[test]
    fn test_edge_record_size() {
        assert_eq!(std::mem::size_of::<EdgeRecord>(), 256);
    }

    #[test]
    fn test_node_record_name_roundtrip() {
        let mut rec = NodeRecord::new(BeingId::new());
        rec.set_name("Hello World hello world");
        assert_eq!(rec.get_name(), "Hello World hello world");
    }

    #[test]
    fn test_node_record_long_name_truncation() {
        let mut rec = NodeRecord::new(BeingId::new());
        let long = "a".repeat(100);
        rec.set_name(&long);
        let got = rec.get_name();
        assert_eq!(got.len(), 63);
        assert_eq!(&got, &long[..63]);
    }

    #[test]
    fn test_edge_record_basic() {
        let mut rec = EdgeRecord::empty();
        rec.from_id = BeingId::new();
        rec.to_id = BeingId::new();
        rec.relation_type = 1;
        rec.directed = 1;
        rec.set_name("test_edge");
        assert_eq!(rec.get_name(), "test_edge");
        assert!(!rec.is_empty());
    }

    #[test]
    fn test_node_record_active() {
        let mut rec = NodeRecord::new(BeingId::new());
        assert!(rec.is_active());
        rec.tx_end = 10;
        assert!(!rec.is_active());
    }
}
