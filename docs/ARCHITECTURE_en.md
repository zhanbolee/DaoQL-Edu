<!--
Copyright 2026 Zhanbo Li / Atlas Lee <zhanbo.lee@gmail.com>

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

    http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
-->



# DaoQL-Edu Architecture Design Document

> **Version**: v1.0  
> **Date**: 2026-05-18  
> **Status**: Pending Design Review  
> **License**: BSL 1.1

---

## Table of Contents

1. [Design Goals and Principles](#1-design-goals-and-principles)
2. [Overall Architecture](#2-overall-architecture)
3. [Module Division](#3-module-division)
4. [Core Data Structures](#4-core-data-structures)
5. [Storage Engine Design](#5-storage-engine-design)
6. [Index Design](#6-index-design)
7. [Transaction and WAL Design](#7-transaction-and-wal-design)
8. [Query Engine Design](#8-query-engine-design)
9. [Key Algorithms in Detail](#9-key-algorithms-in-detail)
10. [Interface Definitions](#10-interface-definitions)
11. [File Layout](#11-file-layout)
12. [Risks and Trade-offs](#12-risks-and-trade-offs)

---

## 1. Design Goals and Principles

### 1.1 Goals
- **Education First**: Every design decision must include a "why it was designed this way" annotation
- **Visible Performance**: Retain core performance elements so learners can see "where the optimization is"
- **Concept Focused**: Do not introduce tangential complexity such as distributed systems, permissions, or full-text search

### 1.2 Principles
- **Single crate**: Not a workspace, lowering the barrier to understanding
- **Minimal Abstraction**: Prefer explicit implementations; avoid excessive generics/macros
- **Comments as Documentation**: Key algorithms must have step-by-step derivation comments
- **Unsafe Isolation**: Unsafe code is concentrated in `storage/mmap.rs`; all other modules remain safe

---

## 2. Overall Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                         DaoQL-Edu Engine                             │
├─────────────────────────────────────────────────────────────────────┤
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────────────┐  │
│  │  Fluent API │  │  DSL Parser │  │  Query Planner / Optimizer  │  │
│  │  (Rust API) │  │  (GraphQL-) │  │  (Route + Pushdown)         │  │
│  └──────┬──────┘  └──────┬──────┘  └──────────────┬──────────────┘  │
│         └─────────────────┴────────────────────────┘                 │
│                              │                                       │
│                    ┌─────────┴─────────┐                             │
│                    │   Query Router    │                             │
│                    │ (Determines which │                             │
│                    │  engine to use)   │                             │
│                    └─────────┬─────────┘                             │
│           ┌──────────────────┼──────────────────┐                   │
│           │                  │                  │                   │
│     ┌─────┴─────┐    ┌──────┴──────┐   ┌──────┴──────┐            │
│     │  Graph    │    │  Column     │   │   Vector    │            │
│     │  Engine   │    │  Engine     │   │   Engine    │            │
│     │ (mmap)    │    │ (Raw+Proj)  │   │  (HNSW)     │            │
│     └─────┬─────┘    └──────┬──────┘   └──────┬──────┘            │
│           │                 │                 │                    │
│           └─────────────────┼─────────────────┘                    │
│                             │                                      │
│              ┌──────────────┴──────────────┐                       │
│              │      Transaction Manager    │                       │
│              │  (PerBeingLock + 2PC + WAL) │                       │
│              └──────────────┬──────────────┘                       │
│                             │                                      │
│     ┌───────────────────────┼───────────────────────┐              │
│     │                       │                       │              │
│ ┌───┴────┐          ┌───────┴───────┐      ┌──────┴──────┐       │
│ │ Index  │          │  PageCache    │      │     WAL     │       │
│ │ (redb) │          │ (ClockSweep)  │      │ (GroupCommit│       │
│ │        │          │               │      │  + CRC32)   │       │
│ └────────┘          └───────────────┘      └─────────────┘       │
│                                                                    │
│  ┌─────────────────────────────────────────────────────────────┐  │
│  │              Mmap / File Storage Layer                       │  │
│  │  (nodes.dat, edges.dat, column_chunks/, hnsw/, wal.bin)     │  │
│  └─────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 3. Module Division

### 3.1 Module Hierarchy

```
src/
├── lib.rs              # Public API entry point; re-exports core types
├── error.rs            # Unified error type (thiserror)
├── config.rs           # TOML configuration parsing
│
├── id.rs               # BeingId, DefId, RelationTypeId
├── being.rs            # BeingCore, BeingExt, Being (composite)
├── def.rs              # Def, Field, FieldType, Constraint
├── relation.rs         # Relation, RelationType
├── version.rs          # VersionChain, MVCC helper functions
│
├── storage/
│   ├── mod.rs          # StorageManager interface
│   ├── mmap.rs         # mmap operations (unsafe isolation)
│   ├── page.rs         # Page allocator (within mmap file)
│   └── file.rs         # Column file / chunk file management
│
├── graph/
│   ├── mod.rs          # GraphEngine interface
│   ├── record.rs       # NodeRecord(1536B), EdgeRecord(256B)
│   ├── store.rs        # mmap graph storage
│   ├── traversal.rs    # BFS, DFS, PageRank
│   └── lock.rs         # PerBeingLock management
│
├── column/
│   ├── mod.rs          # ColumnEngine interface
│   ├── raw_layer.rs    # RawLayer (append-only row store)
│   ├── projected_layer.rs # ProjectedLayer (column store)
│   ├── granule.rs      # Granule (64KB block + min/max)
│   ├── skip_index.rs   # SkipIndex (min/max metadata)
│   ├── aggregation.rs  # SIMD aggregation implementation
│   └── compressor.rs   # lz4_flex compression
│
├── vector/
│   ├── mod.rs          # VectorEngine interface
│   ├── hnsw.rs         # HNSW index implementation
│   ├── distance.rs     # SIMD distance calculation (Cosine/L2/Dot)
│   ├── quantization.rs # SQ/BQ quantization
│   └── graph_prior.rs  # Graph prior insertion seed selection
│
├── index/
│   ├── mod.rs          # IndexManager
│   ├── uuid_index.rs   # redb: UUID → NodeOffset
│   └── time_index.rs   # redb: created_at/updated_at range
│
├── wal/
│   ├── mod.rs          # WalWriter interface
│   ├── record.rs       # WalRecord format definition
│   ├── writer.rs       # Double-buffered group commit implementation
│   └── recovery.rs     # Replay on startup
│
├── pagecache/
│   ├── mod.rs          # PageCache interface
│   └── clock_sweep.rs  # Clock Sweep implementation (16 shards)
│
├── transaction/
│   ├── mod.rs          # TxManager, TxId
│   ├── lock_manager.rs # PerBeingLock pool
│   ├── validator.rs    # ConstraintValidator (simplified)
│   └── committer.rs    # Two-phase commit flow
│
├── query/
│   ├── mod.rs          # QueryEngine, QueryPlan
│   ├── router.rs       # Query routing (determines which engine)
│   ├── planner.rs      # Simple query plan generation
│   └── executor.rs     # Executor (iterator pattern)
│
├── dsl/
│   ├── mod.rs          # DSL entry point
│   ├── lexer.rs        # Lexical analysis
│   ├── parser.rs       # Syntax analysis (recursive descent)
│   ├── ast.rs          # AST node definitions
│   └── executor.rs     # AST → query plan conversion
│
└── api/
    ├── mod.rs          # DaoQL struct (engine entry point)
    ├── query_builder.rs # Fluent API query builder
    ├── write_builder.rs # Fluent API write builder
    └── dsl_api.rs      # DSL execution entry point
```

### 3.2 Module Dependency Graph (Simplified)

```
         api/ ←────── User entry point
          │
    ┌─────┴─────┐
    │           │
 query/      transaction/
    │           │
    ├─────┬─────┼─────┬─────┐
    │     │     │     │     │
 graph/ column/ vector/ index/ wal/
    │     │     │       │     │
    └─────┴─────┴───────┴─────┘
              │
         storage/ ──── pagecache/
```

**Dependency Rules**:
- Upper layers may call lower layers; lower layers must not depend on upper layers
- `storage/` is the lowest layer, depended upon by all data engines
- `api/` is the topmost layer, depending on all lower-layer modules
- Modules at the same layer should avoid direct dependencies; coordinate through `query/` or `transaction/`

---

## 4. Core Data Structures

### 4.1 BeingId

```rust
/// Globally unique entity identifier
/// 
/// Design Notes:
/// - Uses UUID v7 (time-sortable UUID), so time-ordered insertions are naturally sorted
/// - shard_id reserved for sharding extension (fixed to 0 in the educational version)
/// - Total size 18 bytes, stored inline in NodeRecord
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct BeingId {
    pub shard_id: u16,      // 2 bytes, fixed to 0 in the educational version
    pub uuid: Uuid,          // 16 bytes, UUID v7
}

impl BeingId {
    pub const SIZE: usize = 18;
    pub const BYTES_SIZE: usize = 18; // Same as SIZE, used for mmap layout
}
```

### 4.2 NodeRecord (mmap Fixed-Length)

```rust
/// Graph node record — fixed length 1536 bytes
///
/// Educational Notes:
/// - Fixed-length design is the core of graph storage: offset = id * 1536, O(1) random access
/// - Version chain pointers stored inline (prev/next), no extra index needed to trace versions
/// - Relation count limit: max 32 outgoing edges + 32 incoming edges (educational version limit)
/// - EdgeRecord stored inline to avoid pointer chasing
#[repr(C, packed)]
pub struct NodeRecord {
    // === Core Metadata (48 bytes) ===
    pub id: BeingId,                    // 18 bytes
    pub def_type_code: u16,             //  2 bytes — Def type encoding
    pub status: u8,                     //  1 byte
    pub _pad1: [u8; 5],                 //  5 bytes — padding for alignment
    pub created_at: i64,                //  8 bytes — creation time (nanoseconds)
    pub updated_at: i64,                //  8 bytes — update time (nanoseconds)
    pub tx_begin: u64,                  //  8 bytes — MVCC begin transaction number
    pub tx_end: u64,                    //  8 bytes — MVCC end transaction number (u64::MAX = active)
    
    // === Version Chain Pointers (16 bytes) ===
    pub prev_version_offset: u64,       //  8 bytes — previous version node offset
    pub next_version_offset: u64,       //  8 bytes — next version node offset
    
    // === Relation Pointers (16 bytes) ===
    pub first_out_edge_offset: u64,     //  8 bytes — first outgoing edge offset
    pub first_in_edge_offset: u64,      //  8 bytes — first incoming edge offset
    
    // === Inline Core Fields (≈256 bytes) ===
    pub name: [u8; 64],                 // 64 bytes — UTF-8 name
    pub code: [u8; 32],                 // 32 bytes — business code
    pub description: [u8; 128],         // 128 bytes — description
    pub weight: f64,                    //  8 bytes — weight
    pub priority: i32,                  //  4 bytes
    pub _pad2: [u8; 4],                 //  4 bytes
    pub category: [u8; 16],             // 16 bytes
    
    // === Dynamic Field Pointer (8 bytes) ===
    /// Offset pointing to external storage for dynamic fields (BeingExt)
    pub ext_offset: u64,                //  8 bytes — 0 = no dynamic fields
    
    // === Embedding Vector Location (8 bytes) ===
    /// Offset pointing to external vector storage
    pub embedding_offset: u64,          //  8 bytes — 0 = no vector
    
    // === Reserved Space (≈1080 bytes) ===
    /// Educational version reserved: for future extensions or storing more inline fields
    pub reserved: [u8; 1080],
}

// Compile-time assertion: ensure struct size is correct
const_assert_eq!(std::mem::size_of::<NodeRecord>(), 1536);
```

### 4.3 EdgeRecord (mmap Fixed-Length)

```rust
/// Graph edge record — fixed length 256 bytes
///
/// Educational Notes:
/// - Edges are also fixed-length records, stored in a separate mmap region
/// - next_out/next_in pointers form adjacency linked lists, enabling index-free adjacency
/// - Each edge stores source/target BeingId for bidirectional validation
#[repr(C, packed)]
pub struct EdgeRecord {
    pub from_id: BeingId,               // 18 bytes
    pub to_id: BeingId,                 // 18 bytes
    pub relation_type: u16,             //  2 bytes
    pub directed: u8,                   //  1 byte
    pub _pad1: [u8; 1],                 //  1 byte
    pub created_at: i64,                //  8 bytes
    pub tx_begin: u64,                  //  8 bytes
    pub tx_end: u64,                    //  8 bytes
    
    // === Adjacency List Pointers (16 bytes) ===
    pub next_out_edge_offset: u64,      // next outgoing edge from same source
    pub next_in_edge_offset: u64,       // next incoming edge to same target
    
    // === Attribute Area (≈176 bytes) ===
    pub name: [u8; 32],
    pub weight: f64,
    pub _pad2: [u8; 136],
}

const_assert_eq!(std::mem::size_of::<EdgeRecord>(), 256);
```

### 4.4 Being (Complete In-Memory Representation)

```rust
/// Complete in-memory Being representation
///
/// Design Notes:
/// - This is the user-facing data structure, assembled from NodeRecord + ext + embedding
/// - When writing, it is split into NodeRecord + BeingExt for separate storage
pub struct Being {
    pub core: BeingCore,
    pub ext: Option<BeingExt>,
    pub embeddings: HashMap<String, Vec<f32>>,
}

pub struct BeingCore {
    pub id: BeingId,
    pub def: String,
    pub status: u8,
    pub name: String,
    pub code: String,
    pub description: String,
    pub weight: f64,
    pub priority: i32,
    pub category: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub tx_begin: u64,
    pub tx_end: u64,
}

pub struct BeingExt {
    pub being_id: BeingId,
    pub timestamp: i64,
    pub dynamic_attrs: HashMap<String, serde_json::Value>,
}
```

### 4.5 Def (Type Definition)

```rust
/// Type definition
///
/// Educational Notes:
/// - Bootstrapping design: Def itself is also a Being (def = "DAO_DEF")
/// - The educational version does not support inheritance; field list is flat
pub struct Def {
    pub id: BeingId,                    // Bootstrapping: Def itself is a Being
    pub name: String,                   // e.g. "Order"
    pub fields: Vec<Field>,
    pub created_at: i64,
}

pub struct Field {
    pub name: String,
    pub field_type: FieldType,
    pub required: bool,
    pub default_value: Option<serde_json::Value>,
    pub min: Option<f64>,
    pub max: Option<f64>,
}

pub enum FieldType {
    String,
    Int,
    Float,
    Bool,
    Array(Box<FieldType>),
    Map(Box<FieldType>, Box<FieldType>),
}
```

### 4.6 Relation

```rust
/// Relation type definition
pub struct RelationType {
    pub code: u16,
    pub name: String,
    pub directed: bool,
}

/// Relation instance
pub struct Relation {
    pub from_id: BeingId,
    pub to_id: BeingId,
    pub relation_type: u16,
    pub directed: bool,
    pub created_at: i64,
}
```

### 4.7 TxId and Transaction Status

```rust
/// Monotonically increasing transaction ID
///
/// Design Notes:
/// - Uses AtomicU64 to guarantee global monotonic increase
/// - tx_id = 0 is reserved for system use (initialization)
/// - tx_id = u64::MAX is an invalid value
pub type TxId = u64;

/// Active transaction set (for Read Committed isolation)
pub struct ActiveTxSet {
    pub active: RwLock<BTreeSet<TxId>>,
}
```

---

## 5. Storage Engine Design

### 5.1 File Layout

```
data/
├── config.toml           # Runtime configuration
│
├── nodes.dat             # mmap: NodeRecord[] (1536B × N)
├── edges.dat             # mmap: EdgeRecord[] (256B × N)
│
├── index/
│   └── uuid.redb         # redb database: UUID → NodeOffset
│   └── time.redb         # redb database: timestamp → [BeingId]
│
├── columns/
│   ├── raw/
│   │   └── raw-{chunk_id}.dat      # RawLayer chunk files
│   └── projected/
│       └── {def_name}/{field_name}-{chunk_id}.col   # Projected column files
│
├── vectors/
│   └── {field_name}.hnsw           # HNSW graph structure file
│   └── {field_name}.vectors        # Raw vector storage
│
└── wal/
    └── wal.bin             # WAL log file (sequential append)
```

### 5.2 StorageManager

```rust
/// Storage management layer — unified entry point for all data engines
///
/// Responsibilities:
/// - Initialize / open all data files
/// - Allocate new NodeRecord / EdgeRecord offsets
/// - Manage creation of column files and vector files
pub struct StorageManager {
    pub nodes: MmapStore<NodeRecord>,
    pub edges: MmapStore<EdgeRecord>,
    pub column_store: ColumnStore,
    pub vector_store: VectorStore,
    pub wal: WalWriter,
    pub index_db: redb::Database,
}

/// mmap storage abstraction
///
/// Educational Notes:
/// - Unsafe code is concentrated in MmapStore
/// - Exposes safe API: get(offset) → &T, get_mut(offset) → &mut T
/// - Auto-expansion: when the file is full, remap to a larger file
pub struct MmapStore<T: Sized> {
    pub file: File,
    pub mmap: MmapMut,
    pub record_size: usize,
    pub capacity: usize,     // Current number of records that can be held
    pub next_free: usize,    // Next allocatable position
}
```

### 5.3 Column Storage Format

#### RawLayer (Row Store)

```
Chunk File Format:
┌─────────────────────────────────────────────────────────┐
│ Header (256 bytes)                                       │
│   magic: [u8; 4] = b"RAWH"                              │
│   version: u32 = 1                                      │
│   def_type_code: u16                                    │
│   chunk_id: u64                                         │
│   record_count: u32                                     │
│   data_offset: u32                                      │
│   compressed_size: u64                                  │
│   uncompressed_size: u64                                │
│   ... reserved ...                                      │
├─────────────────────────────────────────────────────────┤
│ Data Section (lz4 compressed)                            │
│   [record_len: u32][postcard_blob: ...]                 │
│   [record_len: u32][postcard_blob: ...]                 │
│   ...                                                   │
└─────────────────────────────────────────────────────────┘
```

#### ProjectedLayer (Column Store)

```
Column File Format:
┌─────────────────────────────────────────────────────────┐
│ Header (256 bytes)                                       │
│   magic: [u8; 4] = b"COLH"                              │
│   version: u32 = 1                                      │
│   field_type: u8 (String/Int/Float/Bool)                │
│   granule_count: u32                                    │
│   total_values: u32                                     │
│   ... reserved ...                                      │
├─────────────────────────────────────────────────────────┤
│ Granule Index (N × 32 bytes)                             │
│   offset: u64, count: u32, min_val: [u8; 8], max_val: [u8; 8] │
├─────────────────────────────────────────────────────────┤
│ Granule Data (N blocks, 64KB each)                       │
│   [lz4 compressed column values]                        │
└─────────────────────────────────────────────────────────┘
```

---

## 6. Index Design

### 6.1 UUID → NodeOffset Index (redb)

```rust
/// Strongly consistent index: updated synchronously when the write transaction completes
///
/// Educational Notes:
/// - Uses redb (pure Rust B+Tree), no external process needed
/// - After each transaction commit, writes to redb synchronously
/// - On read, first query redb for offset, then mmap access NodeRecord
pub struct UuidIndex {
    pub db: redb::Database,
    pub table: redb::Table<&'static str, u64>,  // key: UUID string, value: offset
}
```

### 6.2 Time Range Index (redb)

```rust
/// Secondary index: created_at / updated_at range queries
///
/// Design Notes:
/// - Uses redb's MultiMap feature: timestamp → [BeingId]
/// - Range query: db.range(min..max) → iterate matched BeingId lists
/// - Eventual consistency: updated asynchronously in batches (transactions are not blocked)
pub struct TimeIndex {
    pub db: redb::Database,
    pub table: redb::Table<i64, Vec<u8>>,  // key: timestamp, value: BeingId list
}
```

### 6.3 Skip Index (Inline in Columns)

```rust
/// Granule-level min/max metadata
///
/// Educational Notes:
/// - Each Granule (64KB block) maintains min/max
/// - During queries, check min/max first; skip Granules that don't match
/// - This is one of the core optimizations of columnar storage
#[derive(Clone, Copy)]
pub struct GranuleMeta {
    pub offset: u64,            // Data offset
    pub count: u32,             // Record count
    pub min_val: [u8; 8],       // 8 bytes is enough to store i64/f64
    pub max_val: [u8; 8],
}

pub struct SkipIndex {
    pub granules: Vec<GranuleMeta>,
}
```

---

## 7. Transaction and WAL Design

### 7.1 Transaction Flow

```
User Call:
  daoql.write(being)?
    │
    ▼
TxManager::begin()
  1. Allocate TxId (AtomicU64++)
  2. Register in ActiveTxSet
    │
    ▼
Transaction::execute()
  1. Parse changes: Create / Update / Relate
  2. Sort by BeingId (to prevent deadlocks)
  3. Acquire PerBeingLock in batch (write lock)
    │
    ▼
  4. Pre-write WAL
     ┌─────────────────────────────────────────────┐
     │ WAL Record: [magic][len][seq][payload][crc] │
     │ payload = postcard(TransactionPayload)        │
     └─────────────────────────────────────────────┘
    │
    ▼
  5. Execute changes
     - Graph: allocate NodeRecord/EdgeRecord, fill fields
     - Column: append to RawLayer
     - Vector: update HNSW
     - Index: update redb
    │
    ▼
  6. Release locks → remove from ActiveTxSet
  7. Return result
```

### 7.2 WAL Format

```rust
/// WAL record format
///
/// File structure: sequential append, each record independently parseable
///
/// [magic: 4 bytes] = b"WAL\x01"
/// [payload_len: 4 bytes] — little-endian
/// [seq: 8 bytes] — monotonically increasing sequence number
/// [payload: N bytes] — postcard-serialized TransactionPayload
/// [crc32: 4 bytes] — CRC32 checksum of payload
///
/// Educational Notes:
/// - magic is used to identify the file format
/// - seq is used during crash recovery to determine the last valid record
/// - crc32 detects write corruption
/// - Each record is independently parseable, facilitating partial recovery
pub struct WalRecord {
    pub magic: [u8; 4],
    pub payload_len: u32,
    pub seq: u64,
    pub payload: Vec<u8>,
    pub crc32: u32,
}

pub struct TransactionPayload {
    pub tx_id: TxId,
    pub ops: Vec<Op>,
}

pub enum Op {
    CreateBeing { being: Being },
    UpdateBeing { id: BeingId, updates: Vec<FieldUpdate> },
    CreateRelation { relation: Relation },
}
```

### 7.3 Double-Buffered Group Commit

```rust
/// WalWriter — double-buffered group commit
///
/// Educational Notes:
/// - Buffer A: foreground thread appends WAL records (lock-free)
/// - Buffer B: background thread fsyncs to disk
/// - Switch condition: Buffer A is full (default 4MB) or timeout (default 10ms)
/// - When switching, blocks foreground until Buffer B fsync completes
///
/// Why use double buffering?
/// - Single buffer: fsync on every write → extremely low throughput
/// - Double buffer: batch fsync, reducing system calls, improving throughput 10-100x
pub struct WalWriter {
    pub file: File,
    pub buffer_a: Vec<u8>,       // Foreground append buffer
    pub buffer_b: Vec<u8>,       // Background fsync buffer
    pub seq: AtomicU64,
    pub switch_threshold: usize, // Default 4MB
    pub flush_interval: Duration, // Default 10ms
}
```

---

## 8. Query Engine Design

### 8.1 Query Routing

```rust
/// Query router — determines which engine a query should use
///
/// Routing rules:
/// - Point query (by ID) → Graph Engine (mmap direct access)
/// - With relation traversal → Graph Engine (BFS/DFS)
/// - Column scan + filter → Column Engine (projected columns preferred)
/// - Aggregation (SUM/AVG/GROUP BY) → Column Engine (SIMD)
/// - Vector similarity → Vector Engine (HNSW)
/// - Hybrid query → multi-engine result merge
pub struct QueryRouter;

impl QueryRouter {
    pub fn route(&self, query: &Query) -> EngineRoute {
        match query {
            Query::Point(id) => EngineRoute::Graph,
            Query::WithRelations { .. } => EngineRoute::Graph,
            Query::Scan { aggregate: true, .. } => EngineRoute::Column,
            Query::Scan { .. } => EngineRoute::Column,
            Query::Similar { .. } => EngineRoute::Vector,
            Query::Hybrid { .. } => EngineRoute::Multi,
        }
    }
}
```

### 8.2 Executor (Iterator Pattern)

```rust
/// Generic query result iterator
///
/// Design Notes:
/// - All engines return a unified iterator for easy upper-layer composition
/// - Lazy evaluation: data is not actually read until next() is called
/// - Teaching focus: demonstrating the "iterator adapter pattern" in query engines
pub struct QueryResult {
    pub inner: Box<dyn Iterator<Item = Result<Being, DaoQLError>>>,
}

impl QueryResult {
    pub fn filter(self, f: impl Fn(&Being) -> bool) -> Self { ... }
    pub fn limit(self, n: usize) -> Self { ... }
    pub fn map(self, f: impl Fn(Being) -> T) -> impl Iterator<Item = T> { ... }
    pub fn collect_vec(self) -> Result<Vec<Being>, DaoQLError> { ... }
}
```

---

## 9. Key Algorithms in Detail

### 9.1 HNSW Insertion Algorithm

```rust
/// HNSW (Hierarchical Navigable Small World) insertion
///
/// Educational Notes:
/// - HNSW is a multi-layer graph structure: higher layers have sparser connections (like a skip list)
/// - During insertion, start from the top layer with greedy nearest-neighbor search, descending layer by layer
/// - ef_construction controls search width during build (larger → higher graph quality, slower)
/// - M controls maximum out-degree per layer (larger → denser connections, more memory)
///
/// Algorithm Complexity:
/// - Search: O(log N) expected
/// - Insert: O(log N × M) expected
/// - Memory: O(N × M × dim × sizeof(f32))
///
/// Parameter Selection (Educational Version):
/// - M = 16 (moderate)
/// - ef_construction = 100 (build quality prioritized)
/// - ef_search = 64 (query speed prioritized)
/// - max_elements = 100,000 (educational scale)
pub fn hnsw_insert(
    &mut self,
    id: BeingId,
    vector: &[f32],
    graph_prior: Option<Vec<BeingId>>,  // Graph prior: adjacent nodes as search seeds
) {
    // 1. Compute insertion level (exponentially decaying distribution)
    let level = self.random_level();
    
    // 2. Top-layer entry point (global nearest neighbor)
    let mut entry_point = self.entry_point;
    
    // 3. Search nearest neighbor layer by layer
    for l in (level + 1)..self.max_level {
        entry_point = self.greedy_search_layer(entry_point, vector, l, ef=1);
    }
    
    // 4. Start from insertion level, connect layer by layer
    for l in (0..=level).rev() {
        // Search for ef_construction nearest neighbors on this layer
        let neighbors = self.search_layer(entry_point, vector, l, ef_construction);
        
        // Limit out-degree to M, using heuristic selection (retain diverse connections)
        let selected = self.select_neighbors_heuristic(&neighbors, M);
        
        // Bidirectional connection
        for neighbor in &selected {
            self.connect(id, neighbor.id, l);
            self.connect(neighbor.id, id, l);
        }
        
        entry_point = selected[0].id;  // Entry point for next layer
    }
    
    // 5. Update global entry point (if inserted to a higher layer)
    if level > self.max_level {
        self.max_level = level;
        self.entry_point = id;
    }
}
```

### 9.2 Clock Sweep Page Cache

```rust
/// Clock Sweep cache eviction algorithm
///
/// Educational Notes:
/// - Compared to LRU, Clock Sweep is simple to implement, lock-friendly, and has low memory overhead
/// - Each cached page has a "reference bit" (ref bit)
/// - The scan pointer loops through all pages:
///   - ref = 1 → set to 0, skip (give a second chance)
///   - ref = 0 → evict this page
/// - 16 shards: divide the cache into 16 independent sub-caches to reduce lock contention
///
/// Why choose Clock Sweep?
/// - Simple implementation (one loop pointer + one bit)
/// - Approximates LRU effect (Second Chance)
/// - High educational value (classic operating systems algorithm)
///
/// Parameters (Educational Version):
/// - Total pages: 4096 pages
/// - Page size: 64KB
/// - Number of shards: 16
/// - Pages per shard: 256
const CACHE_PAGES: usize = 4096;
const PAGE_SIZE: usize = 64 * 1024;
const NUM_SHARDS: usize = 16;

pub struct PageCache {
    pub shards: [CacheShard; NUM_SHARDS],
}

pub struct CacheShard {
    pub pages: Vec<CachePage>,     // Fixed-size array (256 pages)
    pub clock_hand: AtomicUsize,   // Loop pointer
    pub lock: RwLock<()>,          // Shard lock
}

pub struct CachePage {
    pub key: u64,                  // (file_id << 32) | page_offset
    pub data: Vec<u8>,             // PAGE_SIZE data
    pub ref_bit: AtomicBool,      // Reference bit
    pub dirty: AtomicBool,        // Dirty page flag
}

impl PageCache {
    pub fn get(&self, file_id: u32, offset: u64) -> Option<&[u8]> {
        let shard_idx = ((file_id as u64 ^ offset) % NUM_SHARDS as u64) as usize;
        let shard = &self.shards[shard_idx];
        
        let _guard = shard.lock.read();
        
        let key = ((file_id as u64) << 32) | offset;
        if let Some(page) = shard.pages.iter().find(|p| p.key == key) {
            page.ref_bit.store(true, Ordering::Relaxed);
            return Some(&page.data);
        }
        None
    }
    
    pub fn insert(&self, file_id: u32, offset: u64, data: Vec<u8>) {
        let shard_idx = ((file_id as u64 ^ offset) % NUM_SHARDS as u64) as usize;
        let shard = &self.shards[shard_idx];
        
        let _guard = shard.lock.write();
        
        // Clock Sweep to find a victim page
        let start = shard.clock_hand.load(Ordering::Relaxed);
        let mut victim = None;
        
        for i in 0..shard.pages.len() {
            let idx = (start + i) % shard.pages.len();
            let page = &shard.pages[idx];
            
            if page.ref_bit.swap(false, Ordering::Relaxed) == false {
                // Not referenced even after a second chance → evict
                victim = Some(idx);
                break;
            }
        }
        
        let idx = victim.unwrap_or(start); // If all are referenced, force eviction
        shard.pages[idx] = CachePage {
            key: ((file_id as u64) << 32) | offset,
            data,
            ref_bit: AtomicBool::new(true),
            dirty: AtomicBool::new(false),
        };
        
        shard.clock_hand.store((idx + 1) % shard.pages.len(), Ordering::Relaxed);
    }
}
```

### 9.3 SIMD Aggregation (Column Engine)

```rust
/// SIMD aggregation implementation — using `wide` crate's f64x4
///
/// Educational Notes:
/// - Modern CPUs support SIMD (Single Instruction Multiple Data)
/// - f64x4: operates on 4 f64s at once, theoretically 4x speedup
/// - Actual speedup is about 2-3x (limited by memory bandwidth)
/// - Remainder (less than 4) processed with scalar fallback
///
/// Why use `wide` instead of `std::simd`?
/// - `wide` is stable, `std::simd` is still on nightly
/// - `wide` API is more concise, suitable for teaching
use wide::f64x4;

pub fn simd_sum(values: &[f64]) -> f64 {
    let mut sum = f64x4::ZERO;
    let chunks = values.chunks_exact(4);
    let remainder = chunks.remainder();
    
    for chunk in chunks {
        let v = f64x4::from(chunk);
        sum += v;
    }
    
    let mut result = sum.as_array_ref()[0]
        + sum.as_array_ref()[1]
        + sum.as_array_ref()[2]
        + sum.as_array_ref()[3];
    
    for &v in remainder {
        result += v;
    }
    
    result
}

pub fn simd_dot(a: &[f32], b: &[f32]) -> f32 {
    // Similar implementation using f32x8 or f32x4
    // ...
}
```

### 9.4 Group Commit (WAL)

```rust
/// Double-buffered group commit flow
///
/// Educational Notes:
/// - Group commit: multiple transactions' WAL records are flushed to disk in batches, amortizing fsync cost
/// - Double buffering: foreground writes to Buffer A, background flushes Buffer B, no mutual blocking
///
/// Flow:
///   Thread 1 (Foreground): append → Buffer A
///   Thread 2 (Background): fsync Buffer B → clear → swap with A
///
/// Swap conditions:
///   - Buffer A reaches threshold (4MB)
///   - Timeout (10ms)
///   - Explicit flush() call
///
/// Key: when swapping, briefly block foreground to ensure Buffer B has been flushed
impl WalWriter {
    pub fn append(&mut self, record: &WalRecord) {
        // Serialize record
        let bytes = record.serialize();
        
        // Check if swap is needed
        if self.buffer_a.len() + bytes.len() > self.switch_threshold {
            self.swap_buffers();
        }
        
        self.buffer_a.extend_from_slice(&bytes);
    }
    
    fn swap_buffers(&mut self) {
        // Wait for background fsync to complete (if still in progress)
        // Educational version: synchronous wait; production version can use condition variables
        
        // Swap A ↔ B
        std::mem::swap(&mut self.buffer_a, &mut self.buffer_b);
        
        // Start background fsync
        let buf = std::mem::take(&mut self.buffer_b);
        std::thread::spawn(move || {
            self.file.write_all(&buf).unwrap();
            self.file.sync_all().unwrap();  // fsync
        });
    }
}
```

---

## 10. Interface Definitions

### 10.1 Engine Common Traits

```rust
/// Common interface for all data engines
///
/// Educational Notes:
/// - trait is Rust's core abstraction mechanism
/// - Here demonstrating the "unified interface + independent implementation" architecture pattern
pub trait DataEngine: Send + Sync {
    /// Engine name (for logging / debugging)
    fn name(&self) -> &'static str;
    
    /// Initialize (open / create data files)
    fn init(&mut self, path: &Path) -> Result<(), DaoQLError>;
    
    /// Shutdown (flush to disk, release resources)
    fn shutdown(&mut self) -> Result<(), DaoQLError>;
}

/// Graph engine interface
pub trait GraphEngine: DataEngine {
    /// Create node, return offset
    fn create_node(&mut self, being: &BeingCore) -> Result<u64, DaoQLError>;
    
    /// Read node
    fn read_node(&self, offset: u64) -> Result<&NodeRecord, DaoQLError>;
    
    /// Create edge
    fn create_edge(&mut self, relation: &Relation) -> Result<u64, DaoQLError>;
    
    /// BFS traversal
    fn bfs(
        &self,
        start: BeingId,
        depth: usize,
        filter: Option<EdgeFilter>,
    ) -> Result<Vec<BeingId>, DaoQLError>;
    
    /// PageRank
    fn pagerank(&self, iterations: usize, damping: f64) -> Result<HashMap<BeingId, f64>, DaoQLError>;
}

/// Column engine interface
pub trait ColumnEngine: DataEngine {
    /// Append write to RawLayer
    fn append_raw(&mut self, being: &Being) -> Result<(), DaoQLError>;
    
    /// Register projected column
    fn register_projection(&mut self, def: &str, field: &str, field_type: FieldType);
    
    /// Scan
    fn scan(&self, query: &ScanQuery) -> Result<QueryResult, DaoQLError>;
    
    /// Aggregate
    fn aggregate(&self, query: &AggregateQuery) -> Result<AggregateResult, DaoQLError>;
}

/// Vector engine interface
pub trait VectorEngine: DataEngine {
    /// Register vector field
    fn register_field(&mut self, name: &str, dim: usize);
    
    /// Insert vector
    fn insert(&mut self, id: BeingId, field: &str, vector: Vec<f32>) -> Result<(), DaoQLError>;
    
    /// Similarity search
    fn search(
        &self,
        field: &str,
        query: &[f32],
        k: usize,
        ef: usize,
    ) -> Result<Vec<SearchResult>, DaoQLError>;
}
```

### 10.2 Query Builder (Fluent API)

```rust
/// DaoQL engine entry point
pub struct DaoQL {
    pub storage: Arc<StorageManager>,
    pub tx_manager: TxManager,
    pub query_engine: QueryEngine,
    pub dsl_engine: DslEngine,
}

impl DaoQL {
    pub fn open(path: &Path) -> Result<Self, DaoQLError> { ... }
    
    /// Start a query
    pub fn query(&self) -> QueryBuilder { QueryBuilder::new(self) }
    
    /// Write entity
    pub fn write(&self, being: Being) -> Result<BeingId, DaoQLError> { ... }
    
    /// Establish relation
    pub fn relate(
        &self,
        from: BeingId,
        to: BeingId,
        relation_type: u16,
    ) -> Result<(), DaoQLError> { ... }
    
    /// Execute DSL statement
    pub fn execute_dsl(&self, dsl: &str) -> Result<DslResult, DaoQLError> { ... }
}

/// Query builder
pub struct QueryBuilder<'a> {
    daoql: &'a DaoQL,
    target: QueryTarget,
    filters: Vec<Filter>,
    limit: Option<usize>,
    with_relations: Option<usize>,
    history: Option<HistoryMode>,
    aggregate: Option<AggregateConfig>,
}

impl<'a> QueryBuilder<'a> {
    pub fn being(self, id: BeingId) -> Self { ... }
    pub fn scan(self) -> Self { ... }
    pub fn filter(self, f: Filter) -> Self { ... }
    pub fn limit(self, n: usize) -> Self { ... }
    pub fn with_relations(self, depth: usize) -> Self { ... }
    pub fn as_of(self, timestamp: i64) -> Self { ... }
    pub fn history(self, mode: HistoryMode) -> Self { ... }
    pub fn aggregate(self) -> AggregateBuilder<'a> { ... }
    pub fn similar_to(self, embedding: Vec<f32>, k: usize) -> Self { ... }
    
    pub fn execute(self) -> Result<QueryResult, DaoQLError> { ... }
}
```

### 10.3 Error Types

```rust
/// Unified error type
#[derive(thiserror::Error, Debug)]
pub enum DaoQLError {
    #[error("Storage error: {0}")]
    Storage(#[from] StorageError),
    
    #[error("Index error: {0}")]
    Index(#[from] IndexError),
    
    #[error("Transaction error: {0}")]
    Transaction(#[from] TransactionError),
    
    #[error("Query error: {0}")]
    Query(#[from] QueryError),
    
    #[error("DSL parse error: {0}")]
    DslParse(String),
    
    #[error("Configuration error: {0}")]
    Config(String),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] postcard::Error),
    
    #[error("Not Found: Being {0}")]
    NotFound(BeingId),
    
    #[error("Type mismatch: expected {expected}, actual {actual}")]
    TypeMismatch { expected: String, actual: String },
    
    #[error("Constraint violation: {0}")]
    ConstraintViolation(String),
    
    #[error("MVCC conflict: transaction {tx_id} read uncommitted data")]
    MvccConflict { tx_id: TxId },
}
```

---

## 11. File Layout

### 11.1 Project Root Directory

```
DaoQL-Edu/
├── Cargo.toml
├── LICENSE                 # BSL 1.1
├── README.md
│
├── docs/
│   ├── requirements.md     # Requirements refinement document (Phase 1 output)
│   ├── ARCHITECTURE.md     # Architecture design document (Phase 2 output) ← This document
│   └── API.md              # API user manual
│
├── src/
│   ├── lib.rs
│   ├── error.rs
│   ├── config.rs
│   ├── id.rs
│   ├── being.rs
│   ├── def.rs
│   ├── relation.rs
│   ├── version.rs
│   │
│   ├── storage/
│   │   ├── mod.rs
│   │   ├── mmap.rs
│   │   ├── page.rs
│   │   └── file.rs
│   │
│   ├── graph/
│   │   ├── mod.rs
│   │   ├── record.rs
│   │   ├── store.rs
│   │   ├── traversal.rs
│   │   └── lock.rs
│   │
│   ├── column/
│   │   ├── mod.rs
│   │   ├── raw_layer.rs
│   │   ├── projected_layer.rs
│   │   ├── granule.rs
│   │   ├── skip_index.rs
│   │   ├── aggregation.rs
│   │   └── compressor.rs
│   │
│   ├── vector/
│   │   ├── mod.rs
│   │   ├── hnsw.rs
│   │   ├── distance.rs
│   │   ├── quantization.rs
│   │   └── graph_prior.rs
│   │
│   ├── index/
│   │   ├── mod.rs
│   │   ├── uuid_index.rs
│   │   └── time_index.rs
│   │
│   ├── wal/
│   │   ├── mod.rs
│   │   ├── record.rs
│   │   ├── writer.rs
│   │   └── recovery.rs
│   │
│   ├── pagecache/
│   │   ├── mod.rs
│   │   └── clock_sweep.rs
│   │
│   ├── transaction/
│   │   ├── mod.rs
│   │   ├── lock_manager.rs
│   │   ├── validator.rs
│   │   └── committer.rs
│   │
│   ├── query/
│   │   ├── mod.rs
│   │   ├── router.rs
│   │   ├── planner.rs
│   │   └── executor.rs
│   │
│   ├── dsl/
│   │   ├── mod.rs
│   │   ├── lexer.rs
│   │   ├── parser.rs
│   │   ├── ast.rs
│   │   └── executor.rs
│   │
│   └── api/
│       ├── mod.rs
│       ├── query_builder.rs
│       ├── write_builder.rs
│       └── dsl_api.rs
│
├── tests/
│   ├── integration_tests.rs
│   ├── graph_tests.rs
│   ├── column_tests.rs
│   ├── vector_tests.rs
│   ├── transaction_tests.rs
│   └── dsl_tests.rs
│
└── benches/
    └── benchmark.rs
```

---

## 12. Risks and Trade-offs

| Risk | Impact | Mitigation |
|------|--------|------------|
| mmap cross-platform compatibility | Medium | Use `memmap2` crate (cross-platform), test on Windows/Linux/Mac |
| SIMD portability | Low | `wide` crate automatically falls back to scalar, supports x86_64 and aarch64 |
| redb maturity | Medium | Educational version has small data volume, redb v2 is already stable; retain fallback plan |
| HNSW memory usage | Medium | Educational version limits to 100k vectors; provide scalar/binary quantization compression |
| Single crate code bloat | Low | Limit each file to < 1000 lines, clear modular separation |
| Unsafe code safety | Medium | Unsafe concentrated in storage/mmap.rs, boundary checks + detailed comments |

---

*This document will be locked after review approval, serving as the sole basis for Phases 3–5 implementation.*
