<!--
Copyright 2026 Zhanbo Li / Atlas Lee <4859345@qq.com>

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

> **English**: DaoQL Edu —— A Multimodal Data Engine for Education

[![License](https://img.shields.io/badge/License-AGPL%20v3-blue.svg)](./LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.78%2B-orange.svg)](https://www.rust-lang.org)
[![Tests](https://img.shields.io/badge/Tests-133%2F133%20passing-brightgreen.svg)]()

---

Overview

**DaoQL-Edu** 是 [DaoQL](https://github.com/daoql/daoql) 多模态数据引擎的简化教学实现，专为数据库系统课程和自学者设计。它在保持核心架构完整的前提下，移除了工业级复杂度，使学习者能够清晰地理解图引擎、列存引擎、向量引擎和查询引擎的设计原理与实现细节。

**DaoQL-Edu** is a simplified, educational implementation of the [DaoQL](https://github.com/daoql/daoql) multimodal data engine, designed for database systems courses and self-learners. It preserves the core architecture while removing industrial complexities, enabling learners to clearly understand the design principles and implementation details of graph, columnar, vector, and query engines.

Key Features

| Feature | Description |
| --- | --- |
| Multi-Engine Unified | Graph, Column, and Vector engines share the Being primitive with zero-copy cross-engine queries |
| DSL Query Language | DFS 图遍历 / GraphQL-like syntax supporting Filter, Aggregate, vector similarity search, BFS/DFS graph traversal |
| Cross-Engine Nested Queries | Vector → Graph → Column, BFS → Column aggregation, and other multi-engine pipelines |
| SIMD Acceleration | Columnar aggregation uses NEON SIMD (aarch64), skipping graph scans for direct columnar sums |
| HNSW Vector Index | Textbook implementation with HashMap + scalar distance (edu edition), reserving 5–10× optimization headroom for production |
| Transaction & WAL | Dual-buffer WAL + multi-engine atomic commit with crash recovery support |

---

Architecture

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

详细架构设计请参考 [`docs/ARCHITECTURE.md`](./docs/ARCHITECTURE.md)。  
For detailed architecture design, see [`docs/ARCHITECTURE.md`](./docs/ARCHITECTURE.md).

---

Quick Start

Prerequisites

- **Rust** 1.78+ (Edition 2021)
- **平台** / **Platform**: Apple M-series (aarch64) / x86_64 Linux / x86_64 Windows

Build

```bash
git clone https://github.com/daoql/daoql-edu.git
cd daoql-edu
cargo build --release
```

Run Tests

```bash
# 全部测试（133 个）/ All tests (133 total)
cargo test --release

# 基准测试 / Benchmarks
cargo bench
```

Example

```rust
use daoql_edu::{DaoQL, Being};

// 打开数据库 / Open database
let daoql = DaoQL::open("./data")?;

// 创建实体 / Create a Being
let mut alice = Being::new("Alice", "Person");
alice.core.weight = 65.0;  // 属性映射到列存 / Property maps to column store
daoql.write(alice)?;

// DSL 查询 / DSL query
let result = daoql.execute_dsl(
    r#"query { Person(filter: {weight > 60}) { id, name, weight } }"#
)?;

// 向量搜索 / Vector similarity search
daoql.register_vector_field("embedding", 8);
let result = daoql.execute_dsl(
    r#"similar { Article(query: [0.9, 0.8, 0.7, 0.6, 0.1, 0.1, 0.1, 0.1], k: 3) { } }"#
)?;

// 跨引擎聚合：图扫描过滤 + 列存求和 / Cross-engine aggregation
let result = daoql.query()
    .scan("Order")
    .filter("weight", "gt", serde_json::json!(100.0))
    .aggregate("weight", daoql_edu::column::AggregateOp::Sum)
    .execute()?;
```

---

Performance

The educational edition uses standard algorithm implementations (HashMap, scalar distance, row-by-row processing), reserving optimization headroom for the production version:

| Operation | Edu | Production Est. | Baseline |
| --- | --- | --- | --- |
| Point Query | 0.72 µs | ~0.1 µs | SQLite 1.5 µs |
| BFS Traversal | 1.13 ms | ~200 µs | NetworkX 2.8 ms |
| HNSW Search | 299 µs | ~40 µs | Qdrant 363 µs |
| Column Aggregation | 344.7 µs | ~50 µs | Pandas 1.2 ms |
| Write | 1.47 µs/row | ~0.3 µs/row | SQLite 2.1 µs/row |
| Mixed Query | 123.7 µs | ~20 µs | Neo4j + PG 2.5 ms |

完整性能报告请参考 [`docs/benchmark_vs_competitor_comparison.md`](./docs/benchmark_vs_competitor_comparison.md)。  
For the full performance report, see [`docs/benchmark_vs_competitor_comparison.md`](./docs/benchmark_vs_competitor_comparison.md).

---

Project Structure

```
DaoQL-Edu/
├── src/
│   ├── api/              # Fluent API (QueryBuilder / WriteBuilder)
│   ├── being.rs          # Being 原语定义 / Being primitive definition
│   ├── column/           # 列存引擎 (ProjectedLayer + SIMD 聚合)
│   ├── config.rs         # 配置管理 / Configuration management
│   ├── def.rs            # 类型系统 / Type system
│   ├── dsl/              # DSL 查询语言 (Lexer / Parser / Executor)
│   ├── error.rs          # 错误类型 / Error types
│   ├── graph/            # 图引擎 (mmap 存储 + BFS/DFS/PageRank)
│   ├── id.rs             # BeingId (UUID v7)
│   ├── index/            # 索引 (UUID → Offset, redb B+Tree)
│   ├── lib.rs            # 入口与集成测试 / Entry point & integration tests
│   ├── pagecache/        # 页缓存 / Page cache
│   ├── relation.rs       # 关系原语 / Relation primitive
│   ├── storage/          # 存储层 (mmap / 内存池)
│   ├── transaction/      # 事务与 WAL / Transaction & WAL
│   ├── vector/           # 向量引擎 (HNSW 索引)
│   └── version.rs        # 版本管理 / Version management
├── docs/
│   ├── ARCHITECTURE.md                       # 架构设计文档
│   ├── benchmark_vs_competitor_comparison.md # 性能对比报告
│   ├── check-plan.md                         # 测试计划
│   ├── paradigm/
│   │   └── manifesto_draft_zh.md             # 论文草稿
│   └── requirements.md                       # 需求文档
├── benches/
│   └── benchmark.rs      # Criterion 基准测试
├── tests/                # 集成测试
├── Cargo.toml
├── LICENSE               # BSL 1.1
└── README.md             # 本文档 / This document
```

---

Documentation

| Document | Content |
| --- | --- |
| [`docs/ARCHITECTURE.md`](./docs/ARCHITECTURE.md) | Full architecture design with module diagrams, data structures, and algorithm details |
| [`docs/benchmark_vs_competitor_comparison.md`](./docs/benchmark_vs_competitor_comparison.md) | Neo4j / Qdrant / Pandas / Criterion benchmarks vs SQLite / Neo4j / Qdrant / Pandas |
| [`docs/paradigm/manifesto_draft_zh.md`](./docs/paradigm/manifesto_draft_zh.md) | Paper draft: Multimodal Data Engine Paradigm Manifesto |
| [`docs/check-plan.md`](./docs/check-plan.md) | Test coverage plan and checklist |
| [`docs/requirements.md`](./docs/requirements.md) | Functional requirements and acceptance criteria |

---

Edu vs Production

| Dimension | DaoQL-Edu (教学版) | DaoQL (生产版) |
| --- | --- | --- |
| **Goal** | 教学、学习、原理验证 | 工业级生产部署 |
| **Architecture** | 单 Crate，嵌入式 | 分布式，多节点 |
| **HNSW** | HashMap + 标量距离 | Vec 索引 + SIMD + Generation Counter |
| **列存聚合** | 标准循环 | SIMD + 向量化 + 多线程 |
| **图遍历** | DFS | 并行遍历 + 缓存优化 |
| **写入** | 逐条处理 | 批量分配 + 预写日志优化 |
| **全文检索** | ❌ 不包含 | ✅ 支持 |
| **多租户** | ❌ 不包含 | ✅ 支持 |
| **权限系统** | ❌ 不包含 | ✅ 支持 |

---

Contributing

This is an educational project. Issues and PRs are welcome. All code comments and documentation must be provided in both Chinese and English.

---

License

```
Copyright (c) 2026 Zhanbo Li / Atlas Lee <4859345@qq.com>
SPDX-License-Identifier: AGPL-3.0-or-later

This program is licensed under the GNU Affero General Public License v3.0 (or later).
See the LICENSE file or visit <https://www.gnu.org/licenses/agpl-3.0.html>.
```

详见 [`LICENSE`](./LICENSE) 文件。  
See the [`LICENSE`](./LICENSE) file for details.

---

Author

**Zhanbo Li / Atlas Lee** <4859345@qq.com>
