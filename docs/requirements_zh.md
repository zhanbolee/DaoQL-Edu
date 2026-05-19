<!--
Copyright (c) 2026 黎展波 / Atlas Lee <4859345@qq.com>
SPDX-License-Identifier: BSL-1.1

Licensed under the Business Source License, version 1.1 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at:
    https://mariadb.com/bsl11/

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
-->

# DaoQL-Edu 需求精化文档

> **版本**: v1.0  
> **日期**: 2026-05-18  
> **状态**: 已确认  
> **协议**: Business Source License 1.1 (BSL 1.1)

---

## 1. 项目概述

**DaoQL-Edu** 是 **DaoQL** 的教学版简化实现，旨在帮助学习者理解多模态数据引擎的核心原理。

DaoQL 是一个以本体论为先（Ontology-First）的多模态数据平台，基于 `Being`（实体）、`Def`（类型定义）、`Relation`（关系）三个本体论原语统一建模，融合图遍历、列式聚合、文档存储、向量搜索四种数据处理能力。

DaoQL-Edu 保留 DaoQL 的核心架构骨架，移除工业级复杂特性，使代码量可控、概念清晰、易于教学。

---

## 2. 项目目标

### 2.1 教学目标
- 理解**多模态数据引擎**的设计原理（图+列+文档+向量统一调度）
- 理解**本体论优先**的数据建模方式（Being/Def/Relation/Version）
- 理解**ACID 事务**在单进程存储引擎中的实现
- 理解**性能优化**在数据引擎中的应用（SIMD、Skip Index、PageCache、Group Commit）

### 2.2 工程目标
- 单进程、零外部依赖（除标准 Rust crate 外）
- 代码量控制在 ~20,000-25,000 行 Rust
- 每模块配有教学级注释（核心概念、算法复杂度、设计权衡）
- 提供完整的单元测试和集成测试

---

## 3. 核心原语（4个）

### 3.1 Being（实体）

世界上存在的一切事物。每个 Being 拥有：

- **固定核心字段**（`BeingCore`）:
  - `id: BeingId` — 全局唯一标识符（shard_id + UUID v7）
  - `def: String` — 类型定义名（如 `"Order"`）
  - `status: u8` — 状态码
  - `name: String` — 显示名称
  - `created_at / updated_at: i64` — 时间戳（纳秒）
  - `tx_begin / tx_end: u64` — MVCC 事务字段
  - 其他业务字段（code, description, weight 等，共约 20 个）

- **动态扩展属性**（`BeingExt`）:
  - `being_id: BeingId`
  - `timestamp: i64`
  - `dynamic_attrs: HashMap<String, serde_json::Value>` — 无 Schema 约束的键值对

- **嵌入向量**（可选）:
  - `embeddings: HashMap<String, Vec<f32>>` — 多字段命名向量（如 `name_emb`, `desc_emb`）

### 3.2 Def（类型定义）

Being 的结构定义，自举设计：Def 本身也是一种 Being（`def = "DAO_DEF"`）。

- 字段列表：字段名 + 字段类型（String, Int, Float, Bool, Array, Map）
- 约束：required, min, max, default
- **教学版简化**：**不**支持类型继承（extends）、生命周期策略、状态机

### 3.3 Relation（关系）

Being 之间的有向/无向边。

- `type_code: u16` — 关系类型编码（如 `HAS_PARENT=1`, `CREATED_BY=2`）
- `from_id / to_id: BeingId` — 源/目标实体
- `directed: bool` — 是否有向
- **教学版简化**：**不**支持时态有效性（valid_from/valid_until）、weight、metadata

### 3.4 Version（版本）

**与 DaoQL 原版一致**：采用隐式 MVCC 机制。

- 更新 = 追加新版本，旧版本保留
- `tx_begin` / `tx_end` 标记版本生命周期
- 版本链指针：`prev_version` / `next_version` 内联在图节点记录中
- 查询支持：`history: all / last(N) / at(ts)` 时点回溯

---

## 4. 数据处理能力（4种）

### 4.1 图处理（Graph Engine）

- **定长记录存储**：NodeRecord = 1536 bytes, EdgeRecord = 256 bytes
- **mmap 文件映射**：`memmap2::MmapMut`
- **免索引邻接**：边通过 `next_out_edge` / `next_in_edge` 链表内联链接
- **遍历算法**：BFS、DFS
- **图算法**：PageRank（教学版保留此算法作为示例）

### 4.2 列处理（Column Engine）

- **双层架构**：
  - **RawLayer**（文档层）：append-only 行存，postcard + JSON 序列化
  - **ProjectedLayer**（投影列层）：热点字段独立列存，支持 SIMD 聚合
- **Skip Index**：Granule 级 min/max 元数据，查询剪枝
- **聚合函数**：Count, Sum, Avg, Min, Max, Median, Percentile, Stddev, Variance
- **SIMD 加速**：`f64x4` 向量化执行（使用 `wide` crate）

### 4.3 文档处理（Document Engine）

文档能力由 Column Engine 的双层架构提供：
- **RawLayer**：完整动态字段存储（`BeingExt::dynamic_attrs`），行级检索
- **ProjectedLayer**：动态 Schema 热点字段的列式物化
- **教学重点**：展示"文档灵活性 + 列式性能"的统一设计

### 4.4 向量处理（Vector Engine）

- **HNSW 索引**：自研实现，支持多层图结构
- **多字段隔离**：每个嵌入字段拥有独立 HNSw 子图
- **SIMD 距离**：Cosine / L2 / Dot 的 SIMD 加速
- **标量/二进制量化**：教学版保留，展示内存优化技术
- **图先验插入**：利用 Relation 邻接节点作为 HNSW 插入种子
- **RCU 热更新**：`Arc<RwLock<Arc<...>>>` 模式，读无锁

---

## 5. 事务与一致性

### 5.1 ACID 保证

- **原子性**：统一 WAL 覆盖所有引擎变更
- **一致性**：ConstraintValidator 框架（简化版）
- **隔离性**：Read Committed（全局 RwLock + PerBeingLock）
- **持久性**：WAL fsync + Checkpoint

### 5.2 事务实现

- **两阶段提交**：
  1. 按 BeingId 排序加锁（防止死锁）
  2. 批量执行 Create → Update → Relate
  3. WAL commit 持久化
- **PerBeingLock**：每个 Being 一个 `RwLock<()>`，事务批量获取写锁

### 5.3 WAL（Write-Ahead Log）

- **简化版**：单文件顺序追加
- **格式**：`[magic 4B][payload_len 4B][seq 8B][payload][CRC32 4B]`
- **双缓冲组提交**：Buffer A（前台追加）↔ Buffer B（后台 fsync）
- **教学版移除**：复制协议、故障转移、Promote 记录

---

## 6. 查询接口

### 6.1 Fluent API（Rust 链式调用）

```rust
// 点查
daoql.query().being(id).fetch_one()?;

// 带关系遍历
daoql.query().being(id).with_relations(depth: 2).fetch_one()?;

// 列扫描
daoql.query().scan().with_def("Order").filter(|b| b.status == 1).limit(100).execute()?;

// 聚合
daoql.query().aggregate().group_by(["customer_id"]).sum("amount").execute()?;

// 向量相似
daoql.query().similar_to(embedding, k: 10).execute()?;

// 版本回溯
daoql.query().being(id).as_of("2025-01-15T00:00:00Z").fetch_one()?;

// 写入
daoql.write(complete_being)?;
daoql.relate(from_id, to_id, relation_type)?;
```

### 6.2 DSL（声明式查询语言）

保留的 DSL 语法：

```graphql
// 类型定义
define type Order {
    field amount: Float { required }
    field status: Int { default: 0 }
}

// 点查
query { Order(id: "xxx") { id, name, status } }

// 扫描过滤
query { Order(filter: { status: "1" }, limit: 10) { id, name } }

// 聚合
query {
    Order(groupBy: [customer_id]) {
        key: customer_id
        total: sum(amount)
        count: count
    }
}

// 向量搜索
similar { Order(query: emb, k: 10) { id, name, score: _score } }

// 变更
mutation { create Order(input: { amount: 100.0 }) { id } }

// 图算法
analyze { TopPages: PageRank on Order(limit: 10) { id, score } }

// 版本回溯
query { Order(id: "xxx", history: last(5)) { id, name, _version } }
```

**教学版移除的 DSL 语法**：
- `search { ... }` — 全文检索
- `combined { ... }` — 跨引擎组合查询
- `transaction { ... }` — DSL 事务块（保留 Rust API）
- `define type ... extends ...` — 类型继承
- `on_update: ...` — 版本策略（只有默认 append-only）

---

## 7. 索引体系

| 索引 | 存储 | 一致性 | 教学版状态 |
|------|------|--------|-----------|
| UUID → NodeOffset | `redb` B+Tree | 强一致 | ✅ 保留 |
| Time Range | `redb` B+Tree | 强一致 | ✅ 保留 |
| Skip Index (min/max) | 列文件内联 | 最终一致 | ✅ 保留 |
| Tenant / Def / Status 位图 | 内存 HashMap + RoaringBitmap | 最终一致 | ❌ 移除（多租户已移除）|
| 全文倒排索引 | Tantivy | 最终一致 | ❌ 移除 |
| HNSW 向量索引 | 内存 | — | ✅ 保留 |

---

## 8. 基础设施

| 组件 | 教学版状态 | 说明 |
|------|-----------|------|
| **WAL** | ✅ 保留简化版 | 单文件顺序写 + 双缓冲组提交 + CRC32 |
| **PageCache** | ✅ 保留 | Clock Sweep 算法，16 分区（演示缓存淘汰） |
| **Config** | ✅ 保留 | TOML 分层配置 |
| **Metrics** | ❌ 移除 | 不需要 Prometheus |
| **Crypto** | ❌ 移除 | 保留 CRC32 在校验层，移除 SM2/SM3/SM4 |
| **Embedding** | ❌ 移除 | 向量由外部预计算传入，内置 MockEmbedding 用于测试 |
| **Replication** | ❌ 移除 | 主从复制、故障转移全部移除 |
| **Script / Contract** | ❌ 移除 | Rhai 脚本运行时移除 |
| **Semantic / LLM** | ❌ 移除 | NL→DSL、PromptBuilder、OntologyContext 移除 |

---

## 9. 非功能需求

### 9.1 性能
- 点查：< 1ms（UUID → offset → 内存读取）
- 图遍历 30 跳：< 1ms（全内存）
- 向量搜索 TopK 10：< 10ms（10万向量）
- 列聚合 10 万行：< 100ms（投影列 + SIMD）

### 9.2 教学性
- 每模块顶部必须有教学注释（核心概念、算法复杂度、与生产系统的差异）
- 关键算法必须提供逐步推导的注释（如 HNSW 插入、Clock Sweep、Group Commit）
- 测试用例应覆盖边界情况，并作为使用示例

### 9.3 可维护性
- 单 crate 架构（非 workspace），降低学习者理解成本
- 最大文件行数：1000 行（关键模块可适当放宽）
- 函数行数：不超过 80 行
- 避免过度抽象，优先可读性

---

## 10. 技术约束

- **语言**: Rust 2024 Edition
- **架构**: 单进程、零外部服务依赖
- **外部 crate 限制**:
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
- ** unsafe 代码**: 允许用于 mmap 操作和性能关键路径，但必须有详细的安全注释
- **不依赖原 DaoQL**: 不引用、不链接原 DaoQL 的任何 crate，仅作为设计参考

---

## 11. 验收标准

### 11.1 功能验收
- [ ] 可以定义 Def（字段类型、约束）
- [ ] 可以创建 Being（含 core + ext + 可选向量）
- [ ] 可以建立 Relation（有向/无向）
- [ ] 可以图遍历（BFS/DFS）
- [ ] 可以列聚合（SUM/AVG/COUNT + GROUP BY）
- [ ] 可以向量相似搜索（HNSW TopK）
- [ ] 可以版本回溯（history / as_of）
- [ ] DSL 解析与执行完整流水线

### 11.2 测试验收
- [ ] 单元测试覆盖率 > 80%
- [ ] 集成测试覆盖所有 4 种数据能力
- [ ] 压力测试：100万 Being 写入 + 查询通过

### 11.3 文档验收
- [ ] README.md 包含架构概览和快速开始
- [ ] docs/ARCHITECTURE.md 详细设计文档
- [ ] docs/API.md Fluent API 和 DSL 使用手册
- [ ] 每源文件顶部有模块级教学注释

---

## 12. 排除项（明确不实现）

| 特性 | 排除理由 |
|------|---------|
| 全文检索 | 教学价值低，复杂度高（需 Tantivy + Jieba） |
| 多租户 | 简化数据模型，聚焦核心概念 |
| 权限管理（ACL/CBAC） | 非数据引擎核心 |
| 智能合约（Rhai） | 教学版聚焦存储引擎，非业务逻辑层 |
| 语义层/LLM | 非数据引擎核心 |
| 主从复制/故障转移 | 分布式非教学重点 |
| 国密加密 | 可用 CRC32 替代演示校验 |
| 类型继承/状态机 | 简化 Def 模型 |
| 物化视图/自动投影 | 简化列引擎，投影列手动注册 |
| 并发控制（SI/Serializable） | 仅 Read Committed |

---

*本文档经双方确认后锁定，后续修改需经变更评审流程。*
