<!--
Copyright (c) 2026 Zhanbo Li / Atlas Lee <4859345@qq.com>
SPDX-License-Identifier: BSL-1.1

Licensed under the Business Source License, version 1.1 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at:
    https://spdx.org/licenses/BSL-1.1.html

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
-->

# DaoQL-Edu Requirements Refinement Document

> **Version**: v1.0  
> **Date**: 2026-05-18  
> **Status**: Confirmed  
> **License**: Business Source License 1.1 (BSL 1.1)

---

## 1. Project Overview

**DaoQL-Edu** is a simplified, educational implementation of **DaoQL**, designed to help learners understand the core principles of a multimodal data engine.

DaoQL is an Ontology-First multimodal data platform. It unifies modeling around three ontological primitives—`Being` (entity), `Def` (type definition), and `Relation` (relationship)—and integrates four data processing capabilities: graph traversal, columnar aggregation, document storage, and vector search.

DaoQL-Edu preserves the core architectural skeleton of DaoQL while removing industrial-grade complexities, resulting in a manageable codebase with clear concepts suitable for teaching.

---

## 2. Project Objectives

### 2.1 Teaching Objectives
- Understand the design principles of a **multimodal data engine** (unified scheduling of graph + column + document + vector)
- Understand **ontology-first** data modeling (Being/Def/Relation/Version)
- Understand **ACID transaction** implementation in a single-process storage engine
- Understand the application of **performance optimizations** in a data engine (SIMD, Skip Index, PageCache, Group Commit)

### 2.2 Engineering Objectives
- Single-process, zero external dependencies (except standard Rust crates)
- Code size controlled to approximately ~20,000–25,000 lines of Rust
- Each module includes teaching-grade comments (core concepts, algorithmic complexity, design trade-offs)
- Provide comprehensive unit tests and integration tests

---

## 3. Core Primitives (4)

### 3.1 Being (Entity)

Everything that exists in the world. Each Being has:

- **Fixed Core Fields** (`BeingCore`):
  - `id: BeingId` — Globally unique identifier (shard_id + UUID v7)
  - `def: String` — Type definition name (e.g., `"Order"`)
  - `status: u8` — Status code
  - `name: String` — Display name
  - `created_at / updated_at: i64` — Timestamps (nanoseconds)
  - `tx_begin / tx_end: u64` — MVCC transaction fields
  - Other business fields (code, description, weight, etc., approximately 20 in total)

- **Dynamic Extended Attributes** (`BeingExt`):
  - `being_id: BeingId`
  - `timestamp: i64`
  - `dynamic_attrs: HashMap<String, serde_json::Value>` — Schema-free key-value pairs

- **Embedding Vectors** (optional):
  - `embeddings: HashMap<String, Vec<f32>>` — Multi-field named vectors (e.g., `name_emb`, `desc_emb`)

### 3.2 Def (Type Definition)

The structural definition of a Being, designed with bootstrapping in mind: Def itself is also a Being (`def = "DAO_DEF"`).

- Field list: field name + field type (String, Int, Float, Bool, Array, Map)
- Constraints: required, min, max, default
- **Educational simplification**: **No** support for type inheritance (extends), lifecycle policies, or state machines

### 3.3 Relation (Relationship)

Directed or undirected edges between Beings.

- `type_code: u16` — Relationship type code (e.g., `HAS_PARENT=1`, `CREATED_BY=2`)
- `from_id / to_id: BeingId` — Source / target entity
- `directed: bool` — Whether the edge is directed
- **Educational simplification**: **No** support for temporal validity (valid_from/valid_until), weight, or metadata

### 3.4 Version

**Consistent with the original DaoQL**: Uses implicit MVCC.

- Update = append a new version; old versions are retained
- `tx_begin` / `tx_end` mark the version lifecycle
- Version chain pointers: `prev_version` / `next_version` are inlined in the graph node record
- Query support: `history: all / last(N) / at(ts)` for point-in-time retrieval

---

## 4. Data Processing Capabilities (4)

### 4.1 Graph Processing (Graph Engine)

- **Fixed-length record storage**: NodeRecord = 1536 bytes, EdgeRecord = 256 bytes
- **mmap file mapping**: `memmap2::MmapMut`
- **Index-free adjacency**: Edges are inlined and linked via `next_out_edge` / `next_in_edge` linked lists
- **Traversal algorithms**: BFS, DFS
- **Graph algorithms**: PageRank (retained in the educational version as an example)

### 4.2 Column Processing (Column Engine)

- **Two-tier architecture**:
  - **RawLayer** (document layer): append-only row store, postcard + JSON serialization
  - **ProjectedLayer** (projected column layer): hot fields stored in independent columns, supporting SIMD aggregation
- **Skip Index**: Granule-level min/max metadata for query pruning
- **Aggregate functions**: Count, Sum, Avg, Min, Max, Median, Percentile, Stddev, Variance
- **SIMD acceleration**: `f64x4` vectorized execution (using the `wide` crate)

### 4.3 Document Processing (Document Engine)

Document capabilities are provided by the Column Engine's two-tier architecture:
- **RawLayer**: Stores complete dynamic fields (`BeingExt::dynamic_attrs`), row-level retrieval
- **ProjectedLayer**: Columnar materialization of hot fields with dynamic schema
- **Teaching focus**: Demonstrates a unified design of "document flexibility + columnar performance"

### 4.4 Vector Processing (Vector Engine)

- **HNSW index**: Self-implemented, supporting multi-layer graph structure
- **Multi-field isolation**: Each embedding field has its own independent HNSW subgraph
- **SIMD distance**: Cosine / L2 / Dot with SIMD acceleration
- **Scalar / binary quantization**: Retained in the educational version to demonstrate memory optimization techniques
- **Graph-prioritized insertion**: Uses adjacent nodes from Relations as HNSW insertion seeds
- **RCU hot updates**: `Arc<RwLock<Arc<...>>>` pattern, lock-free reads

---

## 5. Transactions and Consistency

### 5.1 ACID Guarantees

- **Atomicity**: A unified WAL covers all engine modifications
- **Consistency**: ConstraintValidator framework (simplified version)
- **Isolation**: Read Committed (global RwLock + PerBeingLock)
- **Durability**: WAL fsync + Checkpoint

### 5.2 Transaction Implementation

- **Two-phase commit**:
  1. Lock acquisition sorted by BeingId (deadlock prevention)
  2. Batch execution: Create → Update → Relate
  3. WAL commit persistence
- **PerBeingLock**: One `RwLock<()>` per Being; transactions acquire write locks in batches

### 5.3 WAL (Write-Ahead Log)

- **Simplified version**: Single-file sequential append
- **Format**: `[magic 4B][payload_len 4B][seq 8B][payload][CRC32 4B]`
- **Double-buffered group commit**: Buffer A (foreground append) ↔ Buffer B (background fsync)
- **Removed in educational version**: Replication protocol, failover, Promote records

---

## 6. Query Interfaces

### 6.1 Fluent API (Rust method chaining)

```rust
// Point query
daoql.query().being(id).fetch_one()?;

// With relationship traversal
daoql.query().being(id).with_relations(depth: 2).fetch_one()?;

// Column scan
daoql.query().scan().with_def("Order").filter(|b| b.status == 1).limit(100).execute()?;

// Aggregation
daoql.query().aggregate().group_by(["customer_id"]).sum("amount").execute()?;

// Vector similarity
daoql.query().similar_to(embedding, k: 10).execute()?;

// Version rollback
daoql.query().being(id).as_of("2025-01-15T00:00:00Z").fetch_one()?;

// Writes
daoql.write(complete_being)?;
daoql.relate(from_id, to_id, relation_type)?;
```

### 6.2 DSL (Declarative Query Language)

Retained DSL syntax:

```graphql
// Type definition
define type Order {
    field amount: Float { required }
    field status: Int { default: 0 }
}

// Point query
query { Order(id: "xxx") { id, name, status } }

// Scan and filter
query { Order(filter: { status: "1" }, limit: 10) { id, name } }

// Aggregation
query {
    Order(groupBy: [customer_id]) {
        key: customer_id
        total: sum(amount)
        count: count
    }
}

// Vector search
similar { Order(query: emb, k: 10) { id, name, score: _score } }

// Mutation
mutation { create Order(input: { amount: 100.0 }) { id } }

// Graph algorithm
analyze { TopPages: PageRank on Order(limit: 10) { id, score } }

// Version rollback
query { Order(id: "xxx", history: last(5)) { id, name, _version } }
```

**DSL syntax removed in the educational version**:
- `search { ... }` — Full-text search
- `combined { ... }` — Cross-engine combined query
- `transaction { ... }` — DSL transaction block (Rust API retained)
- `define type ... extends ...` — Type inheritance
- `on_update: ...` — Versioning policy (only default append-only remains)

---

## 7. Index System

| Index | Storage | Consistency | Educational Version Status |
|-------|---------|-------------|---------------------------|
| UUID → NodeOffset | `redb` B+Tree | Strong consistency | ✅ Retained |
| Time Range | `redb` B+Tree | Strong consistency | ✅ Retained |
| Skip Index (min/max) | Inlined in column files | Eventual consistency | ✅ Retained |
| Tenant / Def / Status bitmap | In-memory HashMap + RoaringBitmap | Eventual consistency | ❌ Removed (multi-tenant already removed) |
| Full-text inverted index | Tantivy | Eventual consistency | ❌ Removed |
| HNSW vector index | In-memory | — | ✅ Retained |

---

## 8. Infrastructure

| Component | Educational Version Status | Description |
|-----------|---------------------------|-------------|
| **WAL** | ✅ Retained (simplified) | Single-file sequential write + double-buffered group commit + CRC32 |
| **PageCache** | ✅ Retained | Clock Sweep algorithm, 16 partitions (demonstrates cache eviction) |
| **Config** | ✅ Retained | TOML hierarchical configuration |
| **Metrics** | ❌ Removed | No Prometheus needed |
| **Crypto** | ❌ Removed | CRC32 retained at the verification layer; SM2/SM3/SM4 removed |
| **Embedding** | ❌ Removed | Vectors are pre-computed externally; built-in MockEmbedding for testing |
| **Replication** | ❌ Removed | Master-slave replication and failover fully removed |
| **Script / Contract** | ❌ Removed | Rhai script runtime removed |
| **Semantic / LLM** | ❌ Removed | NL→DSL, PromptBuilder, OntologyContext removed |

---

## 9. Non-Functional Requirements

### 9.1 Performance
- Point query: < 1 ms (UUID → offset → memory read)
- Graph traversal 30 hops: < 1 ms (fully in-memory)
- Vector search TopK 10: < 10 ms (100k vectors)
- Columnar aggregation 100k rows: < 100 ms (projected columns + SIMD)

### 9.2 Teachability
- Every module must have a teaching comment block at the top (core concepts, algorithmic complexity, differences from production systems)
- Key algorithms must provide step-by-step derivation comments (e.g., HNSW insertion, Clock Sweep, Group Commit)
- Test cases should cover edge cases and serve as usage examples

### 9.3 Maintainability
- Single-crate architecture (not a workspace) to reduce learner comprehension cost
- Maximum file length: 1,000 lines (key modules may be relaxed appropriately)
- Function length: no more than 80 lines
- Avoid excessive abstraction; prioritize readability

---

## 10. Technical Constraints

- **Language**: Rust 2024 Edition
- **Architecture**: Single-process, zero external service dependencies
- **External crate constraints**:
  ```toml
  [dependencies]
  uuid = "1"
  serde = { version = "1", features = ["derive"] }
  serde_json = "1"
  postcard = "1"
  redb = "2"
  roaring = "0.10"
  memmap2 = "0.9"
  crc32fast = "1.3"
  lz4_flex = "0.11"
  wide = "0.7"
  rayon = "1"
  thiserror = "1"
  ```
- **Unsafe code**: Allowed for mmap operations and performance-critical paths, but must include detailed safety comments
- **No dependency on the original DaoQL**: Do not reference or link to any crate from the original DaoQL; use it only as a design reference

---

## 11. Acceptance Criteria

### 11.1 Functional Acceptance
- [ ] Def can be defined (field types, constraints)
- [ ] Being can be created (including core + ext + optional vectors)
- [ ] Relation can be established (directed / undirected)
- [ ] Graph traversal is possible (BFS / DFS)
- [ ] Columnar aggregation is possible (SUM / AVG / COUNT + GROUP BY)
- [ ] Vector similarity search is possible (HNSW TopK)
- [ ] Version rollback is possible (history / as_of)
- [ ] Complete DSL parsing and execution pipeline

### 11.2 Test Acceptance
- [ ] Unit test coverage > 80%
- [ ] Integration tests cover all 4 data capabilities
- [ ] Stress test: 1 million Being writes + queries pass

### 11.3 Documentation Acceptance
- [ ] README.md contains architectural overview and quick start
- [ ] docs/ARCHITECTURE.md detailed design document
- [ ] docs/API.md Fluent API and DSL user manual
- [ ] Module-level teaching comments at the top of every source file

---

## 12. Exclusions (Explicitly Not Implemented)

| Feature | Exclusion Rationale |
|---------|---------------------|
| Full-text search | Low teaching value, high complexity (requires Tantivy + Jieba) |
| Multi-tenancy | Simplifies the data model; focuses on core concepts |
| Access control (ACL/CBAC) | Not a core data engine concern |
| Smart contracts (Rhai) | Educational version focuses on the storage engine, not the business logic layer |
| Semantic layer / LLM | Not a core data engine concern |
| Master-slave replication / failover | Distributed systems are not the teaching focus |
| National cryptography (SM2/SM3/SM4) | CRC32 can be used instead to demonstrate verification |
| Type inheritance / state machines | Simplifies the Def model |
| Materialized views / automatic projection | Simplifies the column engine; projected columns are manually registered |
| Concurrency control (SI / Serializable) | Read Committed only |

---

*This document is locked after mutual confirmation; subsequent modifications require a change review process.*
