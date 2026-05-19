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

# DaoQL-Edu 检查方案

> **版本**: v1.0  
> **日期**: 2026-05-18  
> **状态**: 已确认  
> **协议**: BSL 1.1

---

## 1. 概述

本文档定义 DaoQL-Edu 项目在**阶段4（执行实现）**和**阶段5（检查验证）**阶段的检查标准、测试策略和质量门禁。

**检查目标**：
- 确保代码符合架构设计文档（docs/ARCHITECTURE.md）
- 确保代码符合 Rust 编码规范
- 确保功能覆盖率 > 80%
- 确保关键算法有正确性验证
- 确保性能基准达标

---

## 2. 编码规范检查清单

### 2.1 基础规范（强制）

| # | 检查项 | 标准 | 验证方式 |
|---|--------|------|---------|
| FMT-01 | 代码格式化 | `cargo fmt` 无变更 | `cargo fmt -- --check` |
| FMT-02 | 行宽 | 120 字符 | `rustfmt.toml: max_width = 120` |
| FMT-03 | 缩进 | 4 空格 | `rustfmt.toml: tab_spaces = 4` |
| FMT-04 | 尾逗号 | 多行结构体/枚举使用尾随逗号 | `rustfmt.toml: trailing_comma = "Vertical"` |

### 2.2 Clippy 规则（强制）

```bash
# 检查命令
cargo clippy --all-targets --all-features -- -D warnings \
  -W clippy::pedantic \
  -W clippy::unwrap_used \
  -W clippy::expect_used \
  -W clippy::panic_in_result_fn \
  -W clippy::todo \
  -A clippy::module_name_repetitions \
  -A clippy::missing_errors_doc \
  -A clippy::missing_panics_doc
```

| # | 检查项 | 严重级别 | 说明 |
|---|--------|---------|------|
| CLIP-01 | `unwrap()` / `expect()` 使用 | ⚠️ Warning | 生产代码中禁止裸 unwrap；测试代码允许 |
| CLIP-02 | `panic!()` 在 Result 返回函数中 | ⚠️ Warning | 应返回 Err 而非 panic |
| CLIP-03 | `todo!()` / `unimplemented!()` | ⚠️ Warning | 提交前必须移除或标记为 ISSUE |
| CLIP-04 | 未使用变量/导入 | ❌ Deny | `-D warnings` |
| CLIP-05 | 可优化代码模式 | ℹ️ Pedantic | 如 `filter_map` 替代 `filter().map()` 等 |

### 2.3 文档规范（强制）

| # | 检查项 | 标准 | 验证方式 |
|---|--------|------|---------|
| DOC-01 | 公共 API 文档 | 所有 `pub` 项必须有 rustdoc | `cargo doc` 无 missing-docs warning |
| DOC-02 | 模块级教学注释 | 每个模块顶部 ≥ 3 行注释，说明核心概念 | 人工检查 |
| DOC-03 | 算法注释 | 关键算法（HNSW/ClockSweep/SIMD/GroupCommit）有逐步推导 | 人工检查 |
| DOC-04 | unsafe 块注释 | 每个 `unsafe` 块必须有 `// SAFETY:` 说明 | `grep -r "unsafe" src/ + 人工检查` |

### 2.4 代码结构规范（强制）

| # | 检查项 | 标准 |
|---|--------|------|
| STRUCT-01 | 单文件行数 | ≤ 1000 行（关键模块可放宽至 1200） |
| STRUCT-02 | 单函数行数 | ≤ 80 行（复杂算法可放宽至 120，需注释说明） |
| STRUCT-03 | unsafe 代码隔离 | 仅 `storage/mmap.rs` 和 `vector/distance.rs` 可用 unsafe |
| STRUCT-04 | 模块依赖方向 | 下层模块不可依赖上层（参考 ARCHITECTURE.md 依赖图） |
| STRUCT-05 | 错误处理统一 | 全部使用 `DaoQLError` 枚举，禁止裸 `Box<dyn Error>` |

---

## 3. 测试策略

### 3.1 测试金字塔

```
         ┌─────────┐
         │ E2E/集成 │  ← 15% 覆盖率目标
         │  (慢)   │     tests/integration_*.rs
         ├─────────┤
         │ 集成测试 │  ← 25% 覆盖率目标
         │  (中)   │     tests/*_tests.rs
         ├─────────┤
         │ 单元测试 │  ← 60% 覆盖率目标
         │  (快)   │     src/*/mod.rs 内的 #[cfg(test)]
         └─────────┘
```

### 3.2 单元测试（`#[cfg(test)]`）

**每个模块必须包含的单元测试**：

| 模块 | 必测项 | 最小用例数 |
|------|--------|-----------|
| `id.rs` | BeingId 生成、序列化、比较 | 5 |
| `being.rs` | BeingCore ↔ NodeRecord 转换 | 5 |
| `def.rs` | 字段类型验证、约束检查 | 5 |
| `version.rs` | tx_begin/tx_end 可见性判断 | 5 |
| `storage/mmap.rs` | 分配、读写、扩容 | 5 |
| `graph/record.rs` | NodeRecord/EdgeRecord 大小断言 | 2 |
| `graph/store.rs` | 创建/读取节点、边 | 5 |
| `graph/traversal.rs` | BFS、DFS 正确性 | 5 |
| `column/raw_layer.rs` | 追加、读取、压缩/解压 | 5 |
| `column/projected_layer.rs` | 投影列注册、扫描 | 5 |
| `column/skip_index.rs` | min/max 剪枝正确性 | 5 |
| `column/aggregation.rs` | SUM/AVG/COUNT 正确性，SIMD vs 标量一致性 | 10 |
| `vector/hnsw.rs` | 插入、搜索、召回率 | 10 |
| `vector/distance.rs` | Cosine/L2/Dot 正确性，SIMD vs 标量一致性 | 10 |
| `vector/quantization.rs` | SQ/BQ 精度损失可控 | 5 |
| `index/uuid_index.rs` | 插入、查询、删除 | 5 |
| `wal/writer.rs` | 追加、双缓冲切换、CRC 校验 | 5 |
| `wal/recovery.rs` | 崩溃恢复、幂等回放 | 5 |
| `pagecache/clock_sweep.rs` | 命中、淘汰、并发安全 | 5 |
| `transaction/lock_manager.rs` | 锁排序、死锁避免 | 5 |
| `transaction/committer.rs` | 两阶段提交流程 | 5 |
| `dsl/lexer.rs` | 词法分析正确性 | 10 |
| `dsl/parser.rs` | 语法分析正确性、错误恢复 | 10 |
| `dsl/executor.rs` | AST → 查询计划 → 执行 | 5 |
| `api/query_builder.rs` | 链式 API 组合 | 5 |

### 3.3 集成测试（`tests/`）

| 测试文件 | 测试场景 | 最小用例数 |
|----------|----------|-----------|
| `integration_tests.rs` | 完整生命周期：Def → Being → Relation → Query → Version | 5 |
| `graph_tests.rs` | 图遍历深度、PageRank 收敛 | 5 |
| `column_tests.rs` | 列扫描过滤、聚合 GROUP BY、Skip Index 剪枝效果 | 5 |
| `vector_tests.rs` | HNSW 召回率 > 90%（10万随机向量 Top10） | 3 |
| `transaction_tests.rs` | 并发写入冲突、WAL 恢复、ACID 验证 | 5 |
| `dsl_tests.rs` | 所有 DSL 语法端到端执行 | 10 |

### 3.4 压力测试（`benches/`）

| 基准测试 | 目标 | 规模 |
|----------|------|------|
| `bench_insert.rs` | 批量写入吞吐量 | 100万 Being |
| `bench_graph_traverse.rs` | 图遍历延迟 | 10万节点，平均度 5 |
| `bench_column_aggregate.rs` | 列聚合延迟 | 100万行，GROUP BY 10 组 |
| `bench_vector_search.rs` | 向量搜索延迟 | 10万向量，dim=384，TopK=10 |
| `bench_point_query.rs` | 点查延迟 | 100万 Being，随机查询 |

**性能目标**（参考需求文档 §9.1）：

| 场景 | 目标 | 可接受范围 |
|------|------|-----------|
| 点查 | < 1ms | < 5ms |
| 图遍历 30 跳 | < 1ms | < 5ms |
| 向量搜索 TopK 10 | < 10ms | < 50ms |
| 列聚合 10 万行 | < 100ms | < 500ms |
| 批量写入 1 万条/秒 | — | > 1000 条/秒 |

---

## 4. 质量门禁

### 4.1 提交前检查（Pre-commit）

```bash
#!/bin/bash
# .githooks/pre-commit（或手动执行）

set -e

echo "🔍 代码格式化检查..."
cargo fmt -- --check

echo "🔍 Clippy 检查..."
cargo clippy --all-targets --all-features -- -D warnings \
  -W clippy::unwrap_used \
  -W clippy::expect_used \
  -W clippy::panic_in_result_fn \
  -W clippy::todo

echo "🔍 单元测试..."
cargo test --lib

echo "🔍 文档构建..."
cargo doc --no-deps

echo "✅ 提交前检查通过"
```

### 4.2 合并前检查（Pre-merge）

| # | 检查项 | 通过标准 |
|---|--------|---------|
| PM-01 | 单元测试 | `cargo test --lib` 全部通过 |
| PM-02 | 集成测试 | `cargo test --test '*'` 全部通过 |
| PM-03 | 文档测试 | `cargo test --doc` 全部通过 |
| PM-04 | Clippy | 零 warning（含 pedantic） |
| PM-05 | 格式化 | `cargo fmt -- --check` 通过 |
| PM-06 | 覆盖率 | `cargo tarpaulin --lib` ≥ 80% |
| PM-07 | 基准测试 | 压力测试无 panic，性能在可接受范围内 |

### 4.3 覆盖率工具

```bash
# 使用 cargo-tarpaulin（Linux）
cargo install cargo-tarpaulin
cargo tarpaulin --lib --out Html --out Stdout

# 或 cargo-llvm-cov（跨平台）
cargo install cargo-llvm-cov
cargo llvm-cov --lib --html
```

**覆盖率目标**：
- 整体行覆盖率 ≥ 80%
- 核心引擎（graph/column/vector）行覆盖率 ≥ 85%
- unsafe 代码路径覆盖率 = 100%

---

## 5. 设计符合性检查

### 5.1 架构符合性

| # | 检查项 | 验证方式 |
|---|--------|---------|
| ARCH-01 | 模块目录与 ARCHITECTURE.md §3.1 一致 | `tree src/ && diff docs/ARCHITECTURE.md` |
| ARCH-02 | 模块依赖方向正确（无反向依赖） | `cargo modules generate tree --lib` |
| ARCH-03 | 核心数据结构大小正确 | `assert_eq!(size_of::<NodeRecord>(), 1536)` 编译时断言 |
| ARCH-04 | 文件布局与 ARCHITECTURE.md §11 一致 | `tree .` |

### 5.2 需求追溯矩阵

| 需求 ID | 需求描述 | 设计文档 | 实现模块 | 测试文件 | 状态 |
|---------|---------|---------|---------|---------|------|
| R-3.1 | Being 原语 | §4.1 | `being.rs` | `being_tests.rs` | ⬜ |
| R-3.2 | Def 原语 | §4.4 | `def.rs` | `def_tests.rs` | ⬜ |
| R-3.3 | Relation 原语 | §4.5 | `relation.rs` | `relation_tests.rs` | ⬜ |
| R-3.4 | Version MVCC | §4.2 | `version.rs` | `version_tests.rs` | ⬜ |
| R-4.1 | 图处理 | §5 / §9.1 | `graph/` | `graph_tests.rs` | ⬜ |
| R-4.2 | 列处理 | §5 / §9.3 | `column/` | `column_tests.rs` | ⬜ |
| R-4.3 | 文档处理 | §5 | `column/raw_layer.rs` | `column_tests.rs` | ⬜ |
| R-4.4 | 向量处理 | §5 / §9.1 | `vector/` | `vector_tests.rs` | ⬜ |
| R-5.1 | ACID 事务 | §7 | `transaction/` | `transaction_tests.rs` | ⬜ |
| R-5.2 | WAL | §7.2 / §9.4 | `wal/` | `wal_tests.rs` | ⬜ |
| R-6.1 | Fluent API | §10.2 | `api/` | `api_tests.rs` | ⬜ |
| R-6.2 | DSL | §8 / `dsl/` | `dsl/` | `dsl_tests.rs` | ⬜ |
| R-7.1 | UUID→Offset 索引 | §6.1 | `index/uuid_index.rs` | `index_tests.rs` | ⬜ |
| R-7.2 | Time Range 索引 | §6.2 | `index/time_index.rs` | `index_tests.rs` | ⬜ |
| R-7.3 | Skip Index | §6.3 | `column/skip_index.rs` | `skip_index_tests.rs` | ⬜ |
| R-8.1 | PageCache | §9.2 | `pagecache/` | `pagecache_tests.rs` | ⬜ |
| R-9.1 | 性能基准 | §3.4 | `benches/` | `benchmark.rs` | ⬜ |
| R-11.1 | 验收标准 | — | 全部 | `integration_tests.rs` | ⬜ |

---

## 6. 安全与稳定性检查

### 6.1 unsafe 代码审计

| # | 检查项 | 标准 |
|---|--------|------|
| UNSAFE-01 | unsafe 代码集中 | 仅 `storage/mmap.rs` 和 `vector/distance.rs` |
| UNSAFE-02 | 边界检查 | 每个 unsafe 指针操作前有长度/范围检查 |
| UNSAFE-03 | SAFETY 注释 | 每个 `unsafe` 块前有 `// SAFETY:` 说明 |
| UNSAFE-04 | Miri 检查 | `cargo miri test` 通过内存安全检测 |

```bash
# Miri 检查命令
rustup component add miri
cargo miri test --lib
```

### 6.2 并发安全

| # | 检查项 | 验证方式 |
|---|--------|---------|
| CONC-01 | `Send + Sync` 正确性 | 编译器检查（数据引擎实现 `Send + Sync`） |
| CONC-02 | 锁粒度合理 | 代码审查：PerBeingLock 而非全局锁 |
| CONC-03 | 死锁避免 | 测试：事务按 BeingId 排序加锁 |
| CONC-04 | 数据竞争 | `cargo miri test` + `loom`（可选） |

### 6.3 资源泄漏

| # | 检查项 | 验证方式 |
|---|--------|---------|
| LEAK-01 | 文件句柄 | `cargo test` 后检查 `/proc/self/fd`（Linux） |
| LEAK-02 | mmap 释放 | 进程退出时 munmap（Drop 实现） |
| LEAK-03 | 锁释放 | `Drop` 实现确保 panic 时释放锁 |

---

## 7. 交付检查清单

### 7.1 代码交付

| # | 检查项 | 状态 |
|---|--------|------|
| DLV-01 | `cargo build` 成功 | ⬜ |
| DLV-02 | `cargo test` 全部通过 | ⬜ |
| DLV-03 | `cargo clippy` 零 warning | ⬜ |
| DLV-04 | `cargo fmt --check` 通过 | ⬜ |
| DLV-05 | `cargo doc` 无 missing-docs | ⬜ |
| DLV-06 | 覆盖率 ≥ 80% | ⬜ |
| DLV-07 | Miri 检查通过 | ⬜ |

### 7.2 文档交付

| # | 检查项 | 状态 |
|---|--------|------|
| DLV-08 | README.md（快速开始） | ⬜ |
| DLV-09 | docs/ARCHITECTURE.md（架构设计） | ✅ |
| DLV-10 | docs/API.md（API 手册） | ⬜ |
| DLV-11 | docs/requirements.md（需求文档） | ✅ |
| DLV-12 | 每模块教学注释 | ⬜ |

### 7.3 功能交付

| # | 检查项 | 状态 |
|---|--------|------|
| DLV-13 | Def 定义与约束 | ⬜ |
| DLV-14 | Being 创建（core + ext + embedding） | ⬜ |
| DLV-15 | Relation 创建（有向/无向） | ⬜ |
| DLV-16 | 图遍历（BFS/DFS） | ⬜ |
| DLV-17 | 列聚合（SUM/AVG/COUNT + GROUP BY） | ⬜ |
| DLV-18 | 向量相似搜索（HNSW TopK） | ⬜ |
| DLV-19 | 版本回溯（history / as_of） | ⬜ |
| DLV-20 | DSL 完整流水线 | ⬜ |

---

## 8. 检查执行计划

### 8.1 阶段4（执行）中的检查节奏

```
每实现一个模块后：
  ├─ cargo fmt
  ├─ cargo clippy
  ├─ cargo test --lib（该模块单元测试）
  └─ cargo doc

每实现一个引擎后（graph/column/vector/index/wal）：
  ├─ 上述所有检查
  ├─ cargo test --test（对应集成测试）
  ├─ cargo tarpaulin（覆盖率快照）
  └─ 更新需求追溯矩阵

全部实现完成后：
  ├─ 完整测试套件
  ├─ 压力测试
  ├─ Miri 检查
  ├─ 覆盖率报告
  └─ 设计符合性审查
```

### 8.2 检查工具链

```toml
# Cargo.toml [dev-dependencies]
[dev-dependencies]
criterion = "0.5"          # 基准测试
cargo-tarpaulin = "0.27"   # 覆盖率（Linux 开发机）
proptest = "1.4"           # 属性测试
rand = "0.8"               # 测试数据生成
```

---

*本文档作为阶段4-5的检查依据，执行过程中按节奏逐项验证。*
