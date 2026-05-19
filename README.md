<!--
Copyright 2026 Zhanbo Li / Atlas Lee <zhanbo.lee@hotmail.com>

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



# DaoQL-Edu

> A Multimodal Data Engine for Education

[![License](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](./LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.78%2B-orange.svg)](https://www.rust-lang.org)
[![Tests](https://img.shields.io/badge/Tests-133%2F133%20passing-brightgreen.svg)]()

---

## Overview

**DaoQL-Edu** is a simplified, educational implementation of the [DaoQL](https://github.com/daoql/daoql) multimodal data engine, designed for database systems courses and self-learners. It preserves the core architecture while removing industrial complexities, enabling learners to clearly understand the design principles and implementation details of graph, columnar, vector, and query engines.

---

## Key Features

| Feature | Description |
| --- | --- |
| Multi-Engine Unified | Graph, Column, and Vector engines share the Being primitive with zero-copy cross-engine queries |
| DSL Query Language | GraphQL-like syntax supporting Filter, Aggregate, vector similarity search, BFS/DFS graph traversal |
| Cross-Engine Nested Queries | Vector → Graph → Column, BFS → Column aggregation, and other multi-engine pipelines |
| SIMD Acceleration | Columnar aggregation uses NEON SIMD (aarch64), skipping graph scans for direct columnar sums |
| HNSW Vector Index | Textbook implementation with HashMap + scalar distance (edu edition), reserving 5–10× optimization headroom for production |
| Transaction & WAL | Dual-buffer WAL + multi-engine atomic commit with crash recovery support |

---

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      DaoQL-Edu Engine                        │
├─────────────────────────────────────────────────────────────┤
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │  Fluent API │  │  DSL Parser │  │  Query Router       │  │
│  │  (Rust API) │  │  (GraphQL-) │  │  (Engine Selection) │  │
│  └──────┬──────┘  └──────┬──────┘  └──────────┬──────────┘  │
│         └─────────────────┴────────────────────┘             │
│                              │                               │
│           ┌──────────────────┼──────────────────┐            │
│           │                  │                  │            │
│     ┌─────┴─────┐    ┌──────┴──────┐   ┌──────┴──────┐     │
│     │  Graph    │    │  Column     │   │   Vector    │     │
│     │  Engine   │    │  Engine     │   │   Engine    │     │
│     │  (mmap)   │    │ (Projected) │   │  (HNSW)     │     │
│     └─────┬─────┘    └──────┬──────┘   └──────┬──────┘     │
│           │                 │                 │            │
│           └─────────────────┼─────────────────┘            │
│                             │                              │
│                    ┌────────┴────────┐                     │
│                    │  Storage Layer  │                     │
│                    │ (mmap / redb)   │                     │
│                    └─────────────────┘                     │
└─────────────────────────────────────────────────────────────┘
```

For detailed architecture design, see [`docs/ARCHITECTURE_en.md`](./docs/ARCHITECTURE_en.md).

---

## Quick Start

### Prerequisites

- **Rust** 1.78+ (Edition 2021)
- **Platform**: Apple M-series (aarch64) / x86_64 Linux / x86_64 Windows

### Build

```bash
git clone https://github.com/zhanbolee/DaoQL-Edu.git
cd DaoQL-Edu
cargo build --release
```

### Run Tests

```bash
# All tests (133 total)
cargo test --release

# Benchmarks
cargo bench
```

### Example

```rust
use daoql_edu::{DaoQL, Being};

// Open database
let daoql = DaoQL::open("./data")?;

// Create a Being
let mut alice = Being::new("Alice", "Person");
alice.core.weight = 65.0;  // Property maps to column store
daoql.write(alice)?;

// DSL query
let result = daoql.execute_dsl(
    r#"query { Person(filter: {weight > 60}) { id, name, weight } }"#
)?;

// Vector similarity search
daoql.register_vector_field("embedding", 8);
let result = daoql.execute_dsl(
    r#"similar { Article(query: [0.9, 0.8, 0.7, 0.6, 0.1, 0.1, 0.1, 0.1], k: 3) { } }"#
)?;

// Cross-engine aggregation: graph scan filter + columnar sum
let result = daoql.query()
    .scan("Order")
    .filter("weight", "gt", serde_json::json!(100.0))
    .aggregate("weight", daoql_edu::column::AggregateOp::Sum)
    .execute()?;
```

---

## Performance

The educational edition uses standard algorithm implementations (HashMap, scalar distance, row-by-row processing), reserving optimization headroom for the production version:

| Operation | Edu | Production Est. | Baseline |
| --- | --- | --- | --- |
| Point Query | 0.72 µs | ~0.1 µs | SQLite 1.5 µs |
| BFS Traversal | 1.13 ms | ~200 µs | NetworkX 2.8 ms |
| HNSW Search | 299 µs | ~40 µs | Qdrant 363 µs |
| Column Aggregation | 344.7 µs | ~50 µs | Pandas 1.2 ms |
| Write | 1.47 µs/row | ~0.3 µs/row | SQLite 2.1 µs/row |
| Mixed Query | 123.7 µs | ~20 µs | Neo4j + PG 2.5 ms |

For the full performance report, see [`docs/benchmark_vs_competitor_comparison_en.md`](./docs/benchmark_vs_competitor_comparison_en.md).

---

## Project Structure

```
DaoQL-Edu/
├── src/
│   ├── api/              # Fluent API (QueryBuilder / WriteBuilder)
│   ├── being.rs          # Being primitive definition
│   ├── column/           # Column engine (ProjectedLayer + SIMD aggregation)
│   ├── config.rs         # Configuration management
│   ├── def.rs            # Type system
│   ├── dsl/              # DSL query language (Lexer / Parser / Executor)
│   ├── error.rs          # Error types
│   ├── graph/            # Graph engine (mmap storage + BFS/DFS)
│   ├── id.rs             # BeingId (UUID v7)
│   ├── index/            # Index (UUID → Offset, redb B+Tree)
│   ├── lib.rs            # Entry point & integration tests
│   ├── pagecache/        # Page cache
│   ├── relation.rs       # Relation primitive
│   ├── storage/          # Storage layer (mmap / memory pool)
│   ├── transaction/      # Transaction & WAL
│   ├── vector/           # Vector engine (HNSW index)
│   └── version.rs        # Version management
├── docs/
│   ├── ARCHITECTURE_en.md
│   ├── ARCHITECTURE_zh.md
│   ├── benchmark_vs_competitor_comparison_en.md
│   ├── benchmark_vs_competitor_comparison_zh.md
│   ├── check-plan_en.md
│   ├── check-plan_zh.md
│   ├── paradigm/
│   │   ├── manifesto_draft_en.md
│   │   └── manifesto_draft_zh.md
│   ├── requirements_en.md
│   └── requirements_zh.md
├── benches/
│   └── benchmark.rs      # Criterion benchmarks
├── Cargo.toml
├── LICENSE               # Apache-2.0
└── README.md             # This document
```

---

## Documentation

| Document | Content |
| --- | --- |
| [`docs/ARCHITECTURE_en.md`](./docs/ARCHITECTURE_en.md) | Full architecture design with module diagrams, data structures, and algorithm details |
| [`docs/benchmark_vs_competitor_comparison_en.md`](./docs/benchmark_vs_competitor_comparison_en.md) | Benchmarks vs SQLite / Neo4j / Qdrant / Pandas |
| [`docs/paradigm/manifesto_draft_en.md`](./docs/paradigm/manifesto_draft_en.md) | Paper draft: Data-First Ontology Manifesto |
| [`docs/check-plan_en.md`](./docs/check-plan_en.md) | Test coverage plan and checklist |
| [`docs/requirements_en.md`](./docs/requirements_en.md) | Functional requirements and acceptance criteria |

---

## Edu vs Production

| Dimension | DaoQL-Edu | DaoQL (Production) |
| --- | --- | --- |
| **Goal** | Education, learning, principle validation | Industrial production deployment |
| **Architecture** | Single crate, embedded | Distributed, multi-node |
| **HNSW** | HashMap + scalar distance | Vec index + SIMD + Generation Counter |
| **Column Aggregation** | Standard loop | SIMD + vectorized + multi-thread |
| **Graph Traversal** | DFS | Parallel traversal + cache optimization |
| **Write** | Row-by-row | Batch allocation + WAL optimization |
| **Full-Text Search** | ❌ Not included | ✅ Supported |
| **Multi-Tenancy** | ❌ Not included | ✅ Supported |
| **Auth System** | ❌ Not included | ✅ Supported |

---

## Contributing

This is an educational project. Issues and PRs are welcome.

---

## License

```
Copyright (c) 2026 Zhanbo Li / Atlas Lee <zhanbo.lee@hotmail.com>
SPDX-License-Identifier: Apache-2.0

Licensed under the Apache License, Version 2.0.
See the LICENSE file or visit <https://www.apache.org/licenses/LICENSE-2.0>.
```

---

## Author

**Zhanbo Li / Atlas Lee** <zhanbo.lee@hotmail.com>
