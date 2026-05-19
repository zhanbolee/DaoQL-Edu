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

# DaoQL-Edu 架构设计文档

> **版本**: v1.0  
> **日期**: 2026-05-18  
> **状态**: 设计评审待审  
> **协议**: BSL 1.1

---

## 目录

1. [设计目标与原则](#1-设计目标与原则)
2. [总体架构](#2-总体架构)
3. [模块划分](#3-模块划分)
4. [核心数据结构](#4-核心数据结构)
5. [存储引擎设计](#5-存储引擎设计)
6. [索引设计](#6-索引设计)
7. [事务与WAL设计](#7-事务与wal设计)
8. [查询引擎设计](#8-查询引擎设计)
9. [关键算法详述](#9-关键算法详述)
10. [接口定义](#10-接口定义)
11. [文件布局](#11-文件布局)
12. [风险与取舍](#12-风险与取舍)

---

## 1. 设计目标与原则

### 1.1 目标
- **教学优先**：每个设计决策必须配有"为什么这样设计"的注释
- **性能可见**：保留核心性能要素，使学习者能看到"优化在哪里"
- **概念聚焦**：不引入分布式、权限、全文检索等旁路复杂度

### 1.2 原则
- **单 crate**：非 workspace，降低理解门槛
- **最小抽象**：优先显式实现，避免过度泛型/宏
- **注释即文档**：关键算法必须有逐步推导注释
- ** unsafe 隔离**：unsafe 代码集中在 `storage/mmap.rs`，其余模块保持 safe

---

## 2. 总体架构

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
│                    │ (判定走哪条引擎)   │                             │
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

## 3. 模块划分

### 3.1 模块层级

```
src/
├── lib.rs              # 公共 API 入口，re-export 核心类型
├── error.rs            # 统一错误类型（thiserror）
├── config.rs           # TOML 配置解析
│
├── id.rs               # BeingId, DefId, RelationTypeId
├── being.rs            # BeingCore, BeingExt, Being（组合体）
├── def.rs              # Def, Field, FieldType, Constraint
├── relation.rs         # Relation, RelationType
├── version.rs          # VersionChain, MVCC 辅助函数
│
├── storage/
│   ├── mod.rs          # StorageManager 接口
│   ├── mmap.rs         # mmap 操作（unsafe 隔离）
│   ├── page.rs         # 页分配器（mmap 文件内）
│   └── file.rs         # 列文件 / chunk 文件管理
│
├── graph/
│   ├── mod.rs          # GraphEngine 接口
│   ├── record.rs       # NodeRecord(1536B), EdgeRecord(256B)
│   ├── store.rs        # mmap 图存储
│   ├── traversal.rs    # BFS, DFS, PageRank
│   └── lock.rs         # PerBeingLock 管理
│
├── column/
│   ├── mod.rs          # ColumnEngine 接口
│   ├── raw_layer.rs    # RawLayer（append-only 行存）
│   ├── projected_layer.rs # ProjectedLayer（列存）
│   ├── granule.rs      # Granule（64KB 块 + min/max）
│   ├── skip_index.rs   # SkipIndex（min/max 元数据）
│   ├── aggregation.rs  # SIMD 聚合实现
│   └── compressor.rs   # lz4_flex 压缩
│
├── vector/
│   ├── mod.rs          # VectorEngine 接口
│   ├── hnsw.rs         # HNSW 索引实现
│   ├── distance.rs     # SIMD 距离计算（Cosine/L2/Dot）
│   ├── quantization.rs # SQ/BQ 量化
│   └── graph_prior.rs  # 图先验插入种子选择
│
├── index/
│   ├── mod.rs          # IndexManager
│   ├── uuid_index.rs   # redb: UUID → NodeOffset
│   └── time_index.rs   # redb: created_at/updated_at 范围
│
├── wal/
│   ├── mod.rs          # WalWriter 接口
│   ├── record.rs       # WalRecord 格式定义
│   ├── writer.rs       # 双缓冲组提交实现
│   └── recovery.rs     # 启动时回放
│
├── pagecache/
│   ├── mod.rs          # PageCache 接口
│   └── clock_sweep.rs  # Clock Sweep 实现（16 分区）
│
├── transaction/
│   ├── mod.rs          # TxManager, TxId
│   ├── lock_manager.rs # PerBeingLock 池
│   ├── validator.rs    # ConstraintValidator（简化版）
│   └── committer.rs    # 两阶段提交流程
│
├── query/
│   ├── mod.rs          # QueryEngine, QueryPlan
│   ├── router.rs       # 查询路由（判定走哪条引擎）
│   ├── planner.rs      # 简单查询计划生成
│   └── executor.rs     # 执行器（迭代器模式）
│
├── dsl/
│   ├── mod.rs          # DSL 入口
│   ├── lexer.rs        # 词法分析
│   ├── parser.rs       # 语法分析（递归下降）
│   ├── ast.rs          # AST 节点定义
│   └── executor.rs     # AST → 查询计划转换
│
└── api/
    ├── mod.rs          # DaoQL 结构体（引擎入口）
    ├── query_builder.rs # Fluent API 查询构建器
    ├── write_builder.rs # Fluent API 写入构建器
    └── dsl_api.rs      # DSL 执行入口
```

### 3.2 模块依赖图（简化）

```
         api/ ←────── 用户入口
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

**依赖规则**：
- 上层可以调用下层，下层不可反向依赖
- `storage/` 是最底层，被所有数据引擎依赖
- `api/` 是最顶层，依赖所有下层模块
- 同层模块之间尽量避免直接依赖，通过 `query/` 或 `transaction/` 协调

---

## 4. 核心数据结构

### 4.1 BeingId

```rust
/// 全局唯一实体标识符
/// 
/// 设计说明：
/// - 使用 UUID v7（时间排序 UUID），使按时间顺序的插入天然有序
/// - shard_id 预留分片扩展（教学版固定为 0）
/// - 总大小 18 bytes，NodeRecord 中内联存储
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct BeingId {
    pub shard_id: u16,      // 2 bytes，教学版固定为 0
    pub uuid: Uuid,          // 16 bytes，UUID v7
}

impl BeingId {
    pub const SIZE: usize = 18;
    pub const BYTES_SIZE: usize = 18; // 与 SIZE 相同，用于 mmap 布局
}
```

### 4.2 NodeRecord（mmap 定长）

```rust
/// 图节点记录 — 定长 1536 bytes
///
/// 教学说明：
/// - 定长设计是图存储的核心：offset = id * 1536，O(1) 随机访问
/// - 版本链指针内联存储（prev/next），无需额外索引即可回溯版本
/// - Relation 计数限制：最多 32 条出边 + 32 条入边（教学版限制）
/// - 内联存储 EdgeRecord，避免指针跳转
#[repr(C, packed)]
pub struct NodeRecord {
    // === 核心元数据 (48 bytes) ===
    pub id: BeingId,                    // 18 bytes
    pub def_type_code: u16,             //  2 bytes — Def 类型编码
    pub status: u8,                     //  1 byte
    pub _pad1: [u8; 5],                 //  5 bytes — padding 对齐
    pub created_at: i64,                //  8 bytes — 创建时间（纳秒）
    pub updated_at: i64,                //  8 bytes — 更新时间（纳秒）
    pub tx_begin: u64,                  //  8 bytes — MVCC 开始事务号
    pub tx_end: u64,                    //  8 bytes — MVCC 结束事务号（u64::MAX = 活跃）
    
    // === 版本链指针 (16 bytes) ===
    pub prev_version_offset: u64,       //  8 bytes — 上一个版本节点偏移
    pub next_version_offset: u64,       //  8 bytes — 下一个版本节点偏移
    
    // === 关系指针 (16 bytes) ===
    pub first_out_edge_offset: u64,     //  8 bytes — 第一条出边偏移
    pub first_in_edge_offset: u64,      //  8 bytes — 第一条入边偏移
    
    // === 内联核心字段 (≈256 bytes) ===
    pub name: [u8; 64],                 // 64 bytes — UTF-8 名称
    pub code: [u8; 32],                 // 32 bytes — 业务编码
    pub description: [u8; 128],         // 128 bytes — 描述
    pub weight: f64,                    //  8 bytes — 权重
    pub priority: i32,                  //  4 bytes
    pub _pad2: [u8; 4],                 //  4 bytes
    pub category: [u8; 16],             // 16 bytes
    
    // === 动态字段指针 (8 bytes) ===
    /// 指向外部存储的动态字段（BeingExt）的 offset
    pub ext_offset: u64,                //  8 bytes — 0 = 无动态字段
    
    // === 嵌入向量的位置信息 (8 bytes) ===
    /// 指向外部向量存储的 offset
    pub embedding_offset: u64,          //  8 bytes — 0 = 无向量
    
    // === 预留空间 (≈1080 bytes) ===
    /// 教学版预留：用于未来扩展，或存储更多内联字段
    pub reserved: [u8; 1080],
}

// 编译时断言：确保结构体大小正确
const_assert_eq!(std::mem::size_of::<NodeRecord>(), 1536);
```

### 4.3 EdgeRecord（mmap 定长）

```rust
/// 图边记录 — 定长 256 bytes
///
/// 教学说明：
/// - 边也是定长记录，存储在独立的 mmap 区域
/// - 通过 next_out/next_in 指针形成邻接链表，实现免索引邻接
/// - 每条边存储源/目标的 BeingId，便于双向验证
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
    
    // === 邻接链表指针 (16 bytes) ===
    pub next_out_edge_offset: u64,      // 同源的下一个出边
    pub next_in_edge_offset: u64,       // 同目标的下一个入边
    
    // === 属性区 (≈176 bytes) ===
    pub name: [u8; 32],
    pub weight: f64,
    pub _pad2: [u8; 136],
}

const_assert_eq!(std::mem::size_of::<EdgeRecord>(), 256);
```

### 4.4 Being（内存完整体）

```rust
/// 内存中的完整 Being 表示
///
/// 设计说明：
/// - 这是用户操作的数据结构，从 NodeRecord + ext + embedding 组装而来
/// - 写入时拆分为 NodeRecord + BeingExt 分别存储
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

### 4.5 Def（类型定义）

```rust
/// 类型定义
///
/// 教学说明：
/// - 自举设计：Def 本身也是一种 Being（def = "DAO_DEF"）
/// - 教学版不支持继承，字段列表是 flat 的
pub struct Def {
    pub id: BeingId,                    // 自举：Def 自己也是 Being
    pub name: String,                   // 如 "Order"
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
/// 关系类型定义
pub struct RelationType {
    pub code: u16,
    pub name: String,
    pub directed: bool,
}

/// 关系实例
pub struct Relation {
    pub from_id: BeingId,
    pub to_id: BeingId,
    pub relation_type: u16,
    pub directed: bool,
    pub created_at: i64,
}
```

### 4.7 TxId 与事务状态

```rust
/// 单调递增的事务 ID
///
/// 设计说明：
/// - 使用 AtomicU64 保证全局单调递增
/// - tx_id = 0 为系统保留（初始化用）
/// - tx_id = u64::MAX 为非法值
pub type TxId = u64;

/// 活跃事务集合（用于 Read Committed 隔离）
pub struct ActiveTxSet {
    pub active: RwLock<BTreeSet<TxId>>,
}
```

---

## 5. 存储引擎设计

### 5.1 文件布局

```
data/
├── config.toml           # 运行时配置
│
├── nodes.dat             # mmap: NodeRecord[]（1536B × N）
├── edges.dat             # mmap: EdgeRecord[]（256B × N）
│
├── index/
│   └── uuid.redb         # redb 数据库：UUID → NodeOffset
│   └── time.redb         # redb 数据库：timestamp → [BeingId]
│
├── columns/
│   ├── raw/
│   │   └── raw-{chunk_id}.dat      # RawLayer chunk 文件
│   └── projected/
│       └── {def_name}/{field_name}-{chunk_id}.col   # 投影列文件
│
├── vectors/
│   └── {field_name}.hnsw           # HNSW 图结构文件
│   └── {field_name}.vectors        # 原始向量存储
│
└── wal/
    └── wal.bin             # WAL 日志文件（顺序追加）
```

### 5.2 StorageManager

```rust
/// 存储管理层 — 所有数据引擎的统一入口
///
/// 职责：
/// - 初始化/打开所有数据文件
/// - 分配新的 NodeRecord / EdgeRecord offset
/// - 管理列文件和向量文件的创建
pub struct StorageManager {
    pub nodes: MmapStore<NodeRecord>,
    pub edges: MmapStore<EdgeRecord>,
    pub column_store: ColumnStore,
    pub vector_store: VectorStore,
    pub wal: WalWriter,
    pub index_db: redb::Database,
}

/// mmap 存储抽象
///
/// 教学说明：
/// - unsafe 代码集中在 MmapStore 中
/// - 对外暴露 safe API：get(offset) → &T, get_mut(offset) → &mut T
/// - 自动扩容：当文件写满时，重新 mmap 更大文件
pub struct MmapStore<T: Sized> {
    pub file: File,
    pub mmap: MmapMut,
    pub record_size: usize,
    pub capacity: usize,     // 当前可容纳的记录数
    pub next_free: usize,    // 下一个可分配位置
}
```

### 5.3 列存储格式

#### RawLayer（行存）

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

#### ProjectedLayer（列存）

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

## 6. 索引设计

### 6.1 UUID → NodeOffset 索引（redb）

```rust
/// 强一致索引：写入事务完成时同步更新
///
/// 教学说明：
/// - 使用 redb（纯 Rust B+Tree），无需外部进程
/// - 每次事务 commit 后，同步写入 redb
/// - 读取时先查 redb 获取 offset，再 mmap 访问 NodeRecord
pub struct UuidIndex {
    pub db: redb::Database,
    pub table: redb::Table<&'static str, u64>,  // key: UUID string, value: offset
}
```

### 6.2 时间范围索引（redb）

```rust
/// 二级索引：created_at / updated_at 范围查询
///
/// 设计说明：
/// - 使用 redb 的 MultiMap 特性：timestamp → [BeingId]
/// - 范围查询：db.range(min..max) → 遍历匹配的 BeingId 列表
/// - 最终一致性：异步批量更新（事务不阻塞）
pub struct TimeIndex {
    pub db: redb::Database,
    pub table: redb::Table<i64, Vec<u8>>,  // key: timestamp, value: BeingId 列表
}
```

### 6.3 Skip Index（列内联）

```rust
/// Granule 级 min/max 元数据
///
/// 教学说明：
/// - 每个 Granule（64KB 块）维护 min/max
/// - 查询时先检查 min/max，不匹配的 Granule 直接跳过
/// - 这是列式存储的核心优化之一
#[derive(Clone, Copy)]
pub struct GranuleMeta {
    pub offset: u64,            // 数据偏移
    pub count: u32,             // 记录数
    pub min_val: [u8; 8],       // 8 bytes 足够存储 i64/f64
    pub max_val: [u8; 8],
}

pub struct SkipIndex {
    pub granules: Vec<GranuleMeta>,
}
```

---

## 7. 事务与WAL设计

### 7.1 事务流程

```
User Call:
  daoql.write(being)?
    │
    ▼
TxManager::begin()
  1. 分配 TxId（AtomicU64++）
  2. 注册到 ActiveTxSet
    │
    ▼
Transaction::execute()
  1. 解析变更：Create / Update / Relate
  2. 按 BeingId 排序（防止死锁）
  3. 批量获取 PerBeingLock（写锁）
    │
    ▼
  4. 预写 WAL
     ┌─────────────────────────────────────────────┐
     │ WAL Record: [magic][len][seq][payload][crc] │
     │ payload = postcard(TransactionPayload)        │
     └─────────────────────────────────────────────┘
    │
    ▼
  5. 执行变更
     - Graph: 分配 NodeRecord/EdgeRecord，填充字段
     - Column: 追加到 RawLayer
     - Vector: 更新 HNSW
     - Index: 更新 redb
    │
    ▼
  6. 释放锁 → 从 ActiveTxSet 移除
  7. 返回结果
```

### 7.2 WAL 格式

```rust
/// WAL 记录格式
///
/// 文件结构：顺序追加，每条记录独立可解析
///
/// [magic: 4 bytes] = b"WAL\x01"
/// [payload_len: 4 bytes] — 小端序
/// [seq: 8 bytes] — 单调递增序列号
/// [payload: N bytes] — postcard 序列化的 TransactionPayload
/// [crc32: 4 bytes] — payload 的 CRC32 校验和
///
/// 教学说明：
/// - magic 用于识别文件格式
/// - seq 用于崩溃恢复时确定最后有效记录
/// - crc32 检测写损坏
/// - 每条记录独立解析，便于部分恢复
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

### 7.3 双缓冲组提交

```rust
/// WalWriter — 双缓冲组提交
///
/// 教学说明：
/// - Buffer A：前台线程追加 WAL 记录（无锁）
/// - Buffer B：后台线程 fsync 到磁盘
/// - 切换条件：Buffer A 满（默认 4MB）或超时（默认 10ms）
/// - 切换时阻塞前台，直到 Buffer B fsync 完成
///
/// 为什么用双缓冲？
/// - 单缓冲：每次写入都 fsync → 吞吐量极低
/// - 双缓冲：批量 fsync，减少系统调用，提升吞吐量 10-100x
pub struct WalWriter {
    pub file: File,
    pub buffer_a: Vec<u8>,       // 前台追加缓冲区
    pub buffer_b: Vec<u8>,       // 后台 fsync 缓冲区
    pub seq: AtomicU64,
    pub switch_threshold: usize, // 默认 4MB
    pub flush_interval: Duration, // 默认 10ms
}
```

---

## 8. 查询引擎设计

### 8.1 查询路由

```rust
/// 查询路由器 — 判定查询走哪条引擎
///
/// 路由规则：
/// - 点查（by ID）→ Graph Engine（mmap 直接访问）
/// - 带关系遍历 → Graph Engine（BFS/DFS）
/// - 列扫描 + 过滤 → Column Engine（投影列优先）
/// - 聚合（SUM/AVG/GROUP BY）→ Column Engine（SIMD）
/// - 向量相似 → Vector Engine（HNSW）
/// - 混合查询 → 多引擎结果合并
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

### 8.2 执行器（迭代器模式）

```rust
/// 通用查询结果迭代器
///
/// 设计说明：
/// - 所有引擎返回统一迭代器，便于上层组合
/// - 惰性求值：直到调用 next() 才真正读取数据
/// - 教学重点：展示"迭代器适配器模式"在查询引擎中的应用
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

## 9. 关键算法详述

### 9.1 HNSW 插入算法

```rust
/// HNSW (Hierarchical Navigable Small World) 插入
///
/// 教学说明：
/// - HNSW 是多层图结构：层数越高，连接越稀疏（类似跳表）
/// - 插入时从顶层开始贪心搜索最近邻，逐层下降
/// - ef_construction 控制构建时的搜索宽度（越大图质量越高，速度越慢）
/// - M 控制每层最大出度（越大连接越密集，内存越大）
///
/// 算法复杂度：
/// - 搜索：O(log N) 期望
/// - 插入：O(log N × M) 期望
/// - 内存：O(N × M × dim × sizeof(f32))
///
/// 参数选择（教学版）：
/// - M = 16（适中）
/// - ef_construction = 100（构建质量优先）
/// - ef_search = 64（查询速度优先）
/// - max_elements = 100,000（教学规模）
pub fn hnsw_insert(
    &mut self,
    id: BeingId,
    vector: &[f32],
    graph_prior: Option<Vec<BeingId>>,  // 图先验：邻接节点作为搜索种子
) {
    // 1. 计算插入层数（指数衰减分布）
    let level = self.random_level();
    
    // 2. 顶层入口点（全局最近邻）
    let mut entry_point = self.entry_point;
    
    // 3. 逐层搜索最近邻
    for l in (level + 1)..self.max_level {
        entry_point = self.greedy_search_layer(entry_point, vector, l, ef=1);
    }
    
    // 4. 从插入层开始，逐层连接
    for l in (0..=level).rev() {
        // 搜索该层的 ef_construction 个最近邻
        let neighbors = self.search_layer(entry_point, vector, l, ef_construction);
        
        // 限制出度为 M，使用启发式选择（保留多样性连接）
        let selected = self.select_neighbors_heuristic(&neighbors, M);
        
        // 双向连接
        for neighbor in &selected {
            self.connect(id, neighbor.id, l);
            self.connect(neighbor.id, id, l);
        }
        
        entry_point = selected[0].id;  // 下一层的入口点
    }
    
    // 5. 更新全局入口点（如果插入到了更高层）
    if level > self.max_level {
        self.max_level = level;
        self.entry_point = id;
    }
}
```

### 9.2 Clock Sweep 页面缓存

```rust
/// Clock Sweep 缓存淘汰算法
///
/// 教学说明：
/// - 相比 LRU，Clock Sweep 实现简单、无锁友好、内存开销低
/// - 每个缓存页有一个 "引用位"（ref bit）
/// - 扫描指针循环遍历所有页：
///   - ref = 1 → 置为 0，跳过（给第二次机会）
///   - ref = 0 → 淘汰该页
/// - 16 分区：将缓存分成 16 个独立子缓存，减少锁竞争
///
/// 为什么选 Clock Sweep？
/// - 实现简单（一个循环指针 + 一个 bit）
/// - 近似 LRU 效果（Second Chance）
/// - 教学价值高（经典操作系统算法）
///
/// 参数（教学版）：
/// - 总页数：4096 页
/// - 页大小：64KB
/// - 分区数：16
/// - 每个分区：256 页
const CACHE_PAGES: usize = 4096;
const PAGE_SIZE: usize = 64 * 1024;
const NUM_SHARDS: usize = 16;

pub struct PageCache {
    pub shards: [CacheShard; NUM_SHARDS],
}

pub struct CacheShard {
    pub pages: Vec<CachePage>,     // 固定大小数组（256 页）
    pub clock_hand: AtomicUsize,   // 循环指针
    pub lock: RwLock<()>,          // 分区锁
}

pub struct CachePage {
    pub key: u64,                  // (file_id << 32) | page_offset
    pub data: Vec<u8>,             // PAGE_SIZE 数据
    pub ref_bit: AtomicBool,      // 引用位
    pub dirty: AtomicBool,        // 脏页标记
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
        
        // Clock Sweep 找淘汰页
        let start = shard.clock_hand.load(Ordering::Relaxed);
        let mut victim = None;
        
        for i in 0..shard.pages.len() {
            let idx = (start + i) % shard.pages.len();
            let page = &shard.pages[idx];
            
            if page.ref_bit.swap(false, Ordering::Relaxed) == false {
                // 第二次机会也没被引用 → 淘汰
                victim = Some(idx);
                break;
            }
        }
        
        let idx = victim.unwrap_or(start); // 如果全部被引用，强制淘汰
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

### 9.3 SIMD 聚合（列引擎）

```rust
/// SIMD 聚合实现 — 使用 `wide` crate 的 f64x4
///
/// 教学说明：
/// - 现代 CPU 支持 SIMD（Single Instruction Multiple Data）
/// - f64x4：一次操作 4 个 f64，理论加速 4x
/// - 实际加速约 2-3x（受内存带宽限制）
/// - 剩余不足 4 的尾部用标量处理
///
/// 为什么用 `wide` 而不是 `std::simd`？
/// - `wide` 稳定可用，`std::simd` 还在 nightly
/// - `wide` API 更简洁，适合教学
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
    // 类似实现，使用 f32x8 或 f32x4
    // ...
}
```

### 9.4 Group Commit（WAL）

```rust
/// 双缓冲组提交流程
///
/// 教学说明：
/// - 组提交：多个事务的 WAL 记录批量刷盘，摊薄 fsync 开销
/// - 双缓冲：前台写 Buffer A，后台刷 Buffer B，不互相阻塞
///
/// 流程：
///   Thread 1 (前台): append → Buffer A
///   Thread 2 (后台): fsync Buffer B → 清空 → 与 A 交换
///
/// 交换条件：
///   - Buffer A 达到阈值（4MB）
///   - 超时（10ms）
///   - 显式调用 flush()
///
/// 关键：交换时需要短暂阻塞前台，确保 Buffer B 已刷完
impl WalWriter {
    pub fn append(&mut self, record: &WalRecord) {
        // 序列化记录
        let bytes = record.serialize();
        
        // 检查是否需要交换
        if self.buffer_a.len() + bytes.len() > self.switch_threshold {
            self.swap_buffers();
        }
        
        self.buffer_a.extend_from_slice(&bytes);
    }
    
    fn swap_buffers(&mut self) {
        // 等待后台 fsync 完成（如果还在进行）
        // 教学版：同步等待，生产版可用条件变量
        
        // 交换 A ↔ B
        std::mem::swap(&mut self.buffer_a, &mut self.buffer_b);
        
        // 启动后台 fsync
        let buf = std::mem::take(&mut self.buffer_b);
        std::thread::spawn(move || {
            self.file.write_all(&buf).unwrap();
            self.file.sync_all().unwrap();  // fsync
        });
    }
}
```

---

## 10. 接口定义

### 10.1 引擎公共 Trait

```rust
/// 所有数据引擎的共同接口
///
/// 教学说明：
/// - trait 是 Rust 的核心抽象机制
/// - 这里展示"统一接口 + 独立实现"的架构模式
pub trait DataEngine: Send + Sync {
    /// 引擎名称（用于日志/调试）
    fn name(&self) -> &'static str;
    
    /// 初始化（打开/创建数据文件）
    fn init(&mut self, path: &Path) -> Result<(), DaoQLError>;
    
    /// 关闭（刷盘、释放资源）
    fn shutdown(&mut self) -> Result<(), DaoQLError>;
}

/// 图引擎接口
pub trait GraphEngine: DataEngine {
    /// 创建节点，返回 offset
    fn create_node(&mut self, being: &BeingCore) -> Result<u64, DaoQLError>;
    
    /// 读取节点
    fn read_node(&self, offset: u64) -> Result<&NodeRecord, DaoQLError>;
    
    /// 创建边
    fn create_edge(&mut self, relation: &Relation) -> Result<u64, DaoQLError>;
    
    /// BFS 遍历
    fn bfs(
        &self,
        start: BeingId,
        depth: usize,
        filter: Option<EdgeFilter>,
    ) -> Result<Vec<BeingId>, DaoQLError>;
    
    /// PageRank
    fn pagerank(&self, iterations: usize, damping: f64) -> Result<HashMap<BeingId, f64>, DaoQLError>;
}

/// 列引擎接口
pub trait ColumnEngine: DataEngine {
    /// 追加写入 RawLayer
    fn append_raw(&mut self, being: &Being) -> Result<(), DaoQLError>;
    
    /// 注册投影列
    fn register_projection(&mut self, def: &str, field: &str, field_type: FieldType);
    
    /// 扫描
    fn scan(&self, query: &ScanQuery) -> Result<QueryResult, DaoQLError>;
    
    /// 聚合
    fn aggregate(&self, query: &AggregateQuery) -> Result<AggregateResult, DaoQLError>;
}

/// 向量引擎接口
pub trait VectorEngine: DataEngine {
    /// 注册向量字段
    fn register_field(&mut self, name: &str, dim: usize);
    
    /// 插入向量
    fn insert(&mut self, id: BeingId, field: &str, vector: Vec<f32>) -> Result<(), DaoQLError>;
    
    /// 相似搜索
    fn search(
        &self,
        field: &str,
        query: &[f32],
        k: usize,
        ef: usize,
    ) -> Result<Vec<SearchResult>, DaoQLError>;
}
```

### 10.2 查询构建器（Fluent API）

```rust
/// DaoQL 引擎入口
pub struct DaoQL {
    pub storage: Arc<StorageManager>,
    pub tx_manager: TxManager,
    pub query_engine: QueryEngine,
    pub dsl_engine: DslEngine,
}

impl DaoQL {
    pub fn open(path: &Path) -> Result<Self, DaoQLError> { ... }
    
    /// 开始查询
    pub fn query(&self) -> QueryBuilder { QueryBuilder::new(self) }
    
    /// 写入实体
    pub fn write(&self, being: Being) -> Result<BeingId, DaoQLError> { ... }
    
    /// 建立关系
    pub fn relate(
        &self,
        from: BeingId,
        to: BeingId,
        relation_type: u16,
    ) -> Result<(), DaoQLError> { ... }
    
    /// 执行 DSL 语句
    pub fn execute_dsl(&self, dsl: &str) -> Result<DslResult, DaoQLError> { ... }
}

/// 查询构建器
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

### 10.3 错误类型

```rust
/// 统一错误类型
#[derive(thiserror::Error, Debug)]
pub enum DaoQLError {
    #[error("存储错误: {0}")]
    Storage(#[from] StorageError),
    
    #[error("索引错误: {0}")]
    Index(#[from] IndexError),
    
    #[error("事务错误: {0}")]
    Transaction(#[from] TransactionError),
    
    #[error("查询错误: {0}")]
    Query(#[from] QueryError),
    
    #[error("DSL 解析错误: {0}")]
    DslParse(String),
    
    #[error("配置错误: {0}")]
    Config(String),
    
    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("序列化错误: {0}")]
    Serialization(#[from] postcard::Error),
    
    #[error("Not Found: Being {0}")]
    NotFound(BeingId),
    
    #[error("类型不匹配: 期望 {expected}, 实际 {actual}")]
    TypeMismatch { expected: String, actual: String },
    
    #[error("约束违反: {0}")]
    ConstraintViolation(String),
    
    #[error("MVCC 冲突: 事务 {tx_id} 读取了未提交数据")]
    MvccConflict { tx_id: TxId },
}
```

---

## 11. 文件布局

### 11.1 项目根目录

```
DaoQL-Edu/
├── Cargo.toml
├── LICENSE                 # BSL 1.1
├── README.md
│
├── docs/
│   ├── requirements.md     # 需求精化文档（阶段1产出）
│   ├── ARCHITECTURE.md     # 架构设计文档（阶段2产出）← 本文档
│   └── API.md              # API 使用手册
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

## 12. 风险与取舍

| 风险 | 影响 | 缓解措施 |
|------|------|---------|
| mmap 跨平台兼容性 | 中 | 使用 `memmap2` crate（跨平台），Windows/Linux/Mac 测试 |
| SIMD 可移植性 | 低 | `wide` crate 自动降级到标量，x86_64 和 aarch64 支持 |
| redb 成熟度 | 中 | 教学版数据量小，redb v2 已稳定；保留 fallback 方案 |
| HNSW 内存占用 | 中 | 教学版限制 10 万向量；提供标量/二进制量化压缩 |
| 单 crate 代码量膨胀 | 低 | 限制每文件 < 1000 行，模块化拆分清晰 |
| unsafe 代码安全性 | 中 | unsafe 集中在 storage/mmap.rs，边界检查 + 详细注释 |

---

*本文档经评审通过后锁定，作为阶段3-5实现的唯一依据。*
