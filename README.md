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

# DaoQL-Edu

> **中文**: DaoQL 教学版 —— 多模态数据引擎教育项目  
> **English**: DaoQL Edu —— A Multimodal Data Engine for Education

[![License](https://img.shields.io/badge/License-BSL%201.1-blue.svg)](./LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.78%2B-orange.svg)](https://www.rust-lang.org)
[![Tests](https://img.shields.io/badge/Tests-133%2F133%20passing-brightgreen.svg)]()

---

## 简介 / Overview

**DaoQL-Edu** 是 [DaoQL](https://github.com/daoql/daoql) 多模态数据引擎的简化教学实现，专为数据库系统课程和自学者设计。它在保持核心架构完整的前提下，移除了工业级复杂度，使学习者能够清晰地理解图引擎、列存引擎、向量引擎和查询引擎的设计原理与实现细节。

**DaoQL-Edu** is a simplified, educational implementation of the [DaoQL](https://github.com/daoql/daoql) multimodal data engine, designed for database systems courses and self-learners. It preserves the core architecture while removing industrial complexities, enabling learners to clearly understand the design principles and implementation details of graph, columnar, vector, and query engines.

### 核心特性 / Key Features

| 特性 / Feature | 说明 / Description |
|---|---|
| **多引擎统一** / Multi-Engine Unified | 图(Graph)、列存(Column)、向量(Vector)三引擎共享 Being 原语，零拷贝跨引擎查询 / Graph, Column, and Vector engines share the Being primitive with zero-copy cross-engine queries |
| **DSL 查询语言** / DSL Query Language | 类 GraphQL 语法，支持 Filter、Aggregate、向量相似搜索、BFS/DFS 图遍历 / GraphQL-like syntax supporting Filter, Aggregate, vector similarity search, BFS/DFS graph traversal |
| **跨引擎嵌套查询** / Cross-Engine Nested Queries | 向量搜索 → 图关系遍历 → 列存属性读取、BFS → 列存聚合等多引擎 pipeline / Vector → Graph → Column, BFS → Column aggregation, and other multi-engine pipelines |
| **SIMD 加速** / SIMD Acceleration | 列存聚合使用 NEON SIMD (aarch64)，跳过图扫描直接列存求和 / Columnar aggregation uses NEON SIMD (aarch64), skipping graph scans for direct columnar sums |
| **HNSW 向量索引** / HNSW Vector Index | 教科书级实现，基于 HashMap + 标量距离（教学版），为生产版保留 5–10× 优化空间 / Textbook implementation with HashMap + scalar distance (edu edition), reserving 5–10× optimization headroom for production |
| **事务与 WAL** / Transaction & WAL | 双缓冲区 WAL + 多引擎原子提交，支持 crash recovery / Dual-buffer WAL + multi-engine atomic commit with crash recovery support |

---

## 架构 / Architecture

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

## 快速开始 / Quick Start

### 环境要求 / Prerequisites

- **Rust** 1.78+ (Edition 2021)
- **平台** / **Platform**: Apple M-series (aarch64) / x86_64 Linux / x86_64 Windows

### 构建 / Build

```bash
git clone https://github.com/daoql/daoql-edu.git
cd daoql-edu
cargo build --release
```

### 运行测试 / Run Tests

```bash
# 全部测试（133 个）/ All tests (133 total)
cargo test --release

# 基准测试 / Benchmarks
cargo bench
```

### 示例代码 / Example

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

## 性能 / Performance

教学版采用标准算法实现（HashMap、标量距离、逐条处理），为生产版保留优化空间：

The educational edition uses standard algorithm implementations (HashMap, scalar distance, row-by-row processing), reserving optimization headroom for the production version:

| 操作 / Operation | 教学版 / Edu | 生产版预估 / Production Est. | 对比基准 / Baseline |
|---|---|---|---|
| 点查 / Point Query | 0.72 µs | ~0.1 µs | SQLite 1.5 µs |
| BFS 遍历 / BFS Traversal | 1.13 ms | ~200 µs | NetworkX 2.8 ms |
| HNSW 向量搜索 / HNSW Search | 299 µs | ~40 µs | Qdrant 363 µs |
| 列存聚合 / Column Aggregation | 344.7 µs | ~50 µs | Pandas 1.2 ms |
| 写入 / Write | 1.47 µs/row | ~0.3 µs/row | SQLite 2.1 µs/row |
| 混合查询 / Mixed Query | 123.7 µs | ~20 µs | Neo4j + PG 2.5 ms |

完整性能报告请参考 [`docs/benchmark_vs_competitor_comparison.md`](./docs/benchmark_vs_competitor_comparison.md)。  
For the full performance report, see [`docs/benchmark_vs_competitor_comparison.md`](./docs/benchmark_vs_competitor_comparison.md).

---

## 项目结构 / Project Structure

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

## 文档 / Documentation

| 文档 / Document | 内容 / Content |
|---|---|
| [`docs/ARCHITECTURE.md`](./docs/ARCHITECTURE.md) | 完整架构设计，含模块关系图、数据结构、算法详述 / Full architecture design with module diagrams, data structures, and algorithm details |
| [`docs/benchmark_vs_competitor_comparison.md`](./docs/benchmark_vs_competitor_comparison.md) | Criterion 实测数据 vs SQLite / Neo4j / Qdrant / Pandas / Criterion benchmarks vs SQLite / Neo4j / Qdrant / Pandas |
| [`docs/paradigm/manifesto_draft_zh.md`](./docs/paradigm/manifesto_draft_zh.md) | 论文草稿：多模态数据引擎范式宣言 / Paper draft: Multimodal Data Engine Paradigm Manifesto |
| [`docs/check-plan.md`](./docs/check-plan.md) | 测试覆盖计划与检查清单 / Test coverage plan and checklist |
| [`docs/requirements.md`](./docs/requirements.md) | 功能需求与验收标准 / Functional requirements and acceptance criteria |

---

## 教学版 vs 生产版 / Edu vs Production

| 维度 / Dimension | DaoQL-Edu (教学版) | DaoQL (生产版) |
|---|---|---|
| **目标** / **Goal** | 教学、学习、原理验证 | 工业级生产部署 |
| **架构** / **Architecture** | 单 Crate，嵌入式 | 分布式，多节点 |
| **HNSW** | HashMap + 标量距离 | Vec 索引 + SIMD + Generation Counter |
| **列存聚合** | 标准循环 | SIMD + 向量化 + 多线程 |
| **图遍历** | 标准 BFS/DFS | 并行遍历 + 缓存优化 |
| **写入** | 逐条处理 | 批量分配 + 预写日志优化 |
| **全文检索** | ❌ 不包含 | ✅ 支持 |
| **多租户** | ❌ 不包含 | ✅ 支持 |
| **权限系统** | ❌ 不包含 | ✅ 支持 |

---

## 贡献 / Contributing

本项目为教学项目，欢迎提交 Issue 和 PR。所有代码注释和文档必须同时提供中文和英文版本。

This is an educational project. Issues and PRs are welcome. All code comments and documentation must be provided in both Chinese and English.

---

## 许可证 / License

```
Copyright (c) 2026 Zhanbo Li / Atlas Lee <4859345@qq.com>
SPDX-License-Identifier: BSL-1.1

Licensed under the Business Source License, version 1.1 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at:

    https://spdx.org/licenses/BSL-1.1.html

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
```

详见 [`LICENSE`](./LICENSE) 文件。  
See the [`LICENSE`](./LICENSE) file for details.

---

## 作者 / Author

**Zhanbo Li / Atlas Lee** <4859345@qq.com>
