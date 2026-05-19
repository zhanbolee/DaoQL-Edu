<!--
Copyright 2026 黎展波 / Atlas Lee <zhanbo.lee@gmail.com>

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

> **中文**: DaoQL 教学版 —— 多模态数据引擎教育项目  

[![License](https://img.shields.io/badge/License-AGPL%20v3-blue.svg)](./LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.78%2B-orange.svg)](https://www.rust-lang.org)
[![Tests](https://img.shields.io/badge/Tests-133%2F133%20passing-brightgreen.svg)]()

---

## 简介

**DaoQL-Edu** 是 [DaoQL](https://github.com/daoql/daoql) 多模态数据引擎的简化教学实现，专为数据库系统课程和自学者设计。它在保持核心架构完整的前提下，移除了工业级复杂度，使学习者能够清晰地理解图引擎、列存引擎、向量引擎和查询引擎的设计原理与实现细节。

**DaoQL-Edu** is a simplified, educational implementation of the [DaoQL](https://github.com/daoql/daoql) multimodal data engine, designed for database systems courses and self-learners. It preserves the core architecture while removing industrial complexities, enabling learners to clearly understand the design principles and implementation details of graph, columnar, vector, and query engines.

### 核心特性

| 特性 | 说明 |
| --- | --- |
| **多引擎统一** | 图(Graph)、列存(Column)、向量(Vector)三引擎共享 Being 原语，零拷贝跨引擎查询 |
| **DSL 查询语言** | 类 GraphQL 语法，支持 Filter、Aggregate、向量相似搜索、BFS |
| **跨引擎嵌套查询** | 向量搜索 → 图关系遍历 → 列存属性读取、BFS → 列存聚合等多引擎 pipeline |
| **SIMD 加速** | 列存聚合使用 NEON SIMD (aarch64)，跳过图扫描直接列存求和 |
| **HNSW 向量索引** | 教科书级实现，基于 HashMap + 标量距离（教学版），为生产版保留 5–10× 优化空间 |
| **事务与 WAL** | 双缓冲区 WAL + 多引擎原子提交，支持 crash recovery |

---

## 架构

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

---

## 快速开始

### 环境要求

### 构建

```bash
git clone https://github.com/daoql/daoql-edu.git
cd daoql-edu
cargo build --release
```

### 运行测试

```bash
# 全部测试（133 个）/ All tests (133 total)
cargo test --release

# 基准测试 / Benchmarks
cargo bench
```

### 示例代码

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

## 性能

教学版采用标准算法实现（HashMap、标量距离、逐条处理），为生产版保留优化空间：

| 操作 | 教学版 | 生产版预估 | 对比基准 |
| --- | --- | --- | --- |
| 点查 | 0.72 µs | ~0.1 µs | SQLite 1.5 µs |
| BFS 遍历 | 1.13 ms | ~200 µs | NetworkX 2.8 ms |
| HNSW 向量搜索 | 299 µs | ~40 µs | Qdrant 363 µs |
| 列存聚合 | 344.7 µs | ~50 µs | Pandas 1.2 ms |
| 写入 | 1.47 µs/row | ~0.3 µs/row | SQLite 2.1 µs/row |
| 混合查询 | 123.7 µs | ~20 µs | Neo4j + PG 2.5 ms |

---

## 项目结构

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

## 文档

| 文档 | 内容 |
| --- | --- |
| [`docs/ARCHITECTURE.md`](./docs/ARCHITECTURE.md) | 完整架构设计，含模块关系图、数据结构、算法详述 |
| [`docs/benchmark_vs_competitor_comparison.md`](./docs/benchmark_vs_competitor_comparison.md) | Criterion 实测数据 vs SQLite |
| [`docs/paradigm/manifesto_draft_zh.md`](./docs/paradigm/manifesto_draft_zh.md) | 论文草稿：多模态数据引擎范式宣言 |
| [`docs/check-plan.md`](./docs/check-plan.md) | 测试覆盖计划与检查清单 |
| [`docs/requirements.md`](./docs/requirements.md) | 功能需求与验收标准 |

---

## 教学版 vs 生产版

| 维度 | DaoQL-Edu (教学版) | DaoQL (生产版) |
| --- | --- | --- |
| **目标** | 教学、学习、原理验证 | 工业级生产部署 |
| **架构** | 单 Crate，嵌入式 | 分布式，多节点 |
| **HNSW** | HashMap + 标量距离 | Vec 索引 + SIMD + Generation Counter |
| **列存聚合** | 标准循环 | SIMD + 向量化 + 多线程 |
| **图遍历** | 标准 BFS | 并行遍历 + 缓存优化 |
| **写入** | 逐条处理 | 批量分配 + 预写日志优化 |
| **全文检索** | ❌ 不包含 | ✅ 支持 |
| **多租户** | ❌ 不包含 | ✅ 支持 |
| **权限系统** | ❌ 不包含 | ✅ 支持 |

---

## 贡献

本项目为教学项目，欢迎提交 Issue 和 PR。所有代码注释和文档必须同时提供中文和英文版本。

---

## 许可证

```
Copyright (c) 2026 黎展波 / Atlas Lee <zhanbo.lee@gmail.com>
SPDX-License-Identifier: AGPL-3.0-or-later

本程序在 GNU Affero 通用公共许可证第3版（或更新版本）条款下发布。
详见 LICENSE 文件或访问 <https://www.gnu.org/licenses/agpl-3.0.html>。
```

---

## 作者

