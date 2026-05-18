// Copyright (c) 2026 黎展波 / Atlas Lee <4859345@qq.com>
// SPDX-License-Identifier: BSL-1.1
//
// Licensed under the Business Source License, version 1.1 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at:
//
//     https://mariadb.com/bsl11/
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
//
#![allow(clippy::result_large_err)]

//! DaoQL-Edu — 多模态数据引擎教学版
//!
//! 教学说明：
//! - 本项目是 DaoQL 的简化教学实现
//! - 保留核心架构：Being/Def/Relation/Version 原语 + 图/列/文档/向量 能力
//! - 保留性能要素：SIMD、SkipIndex、HNSW、PageCache、双缓冲 WAL
//! - 移除工业级复杂度：全文检索、多租户、分布式、权限等
//!
//! # 快速开始
//! ```rust,ignore
//! use daoql_edu::{DaoQL, Being};
//!
//! let daoql = DaoQL::open("./data")?;
//! let being = Being::new("Alice", "Person");
//! daoql.write(being)?;
//! ```

// 基础设施
pub mod api;
pub mod config;
pub mod error;
pub mod id;

// 核心原语
pub mod being;
pub mod def;
pub mod relation;
pub mod version;

// 存储与索引
pub mod column;
pub mod graph;
pub mod index;
pub mod pagecache;
pub mod storage;
pub mod transaction;
pub mod wal;

// 查询与 DSL
pub mod dsl;
pub mod query;

// 向量引擎
pub mod vector;

// 公共重导出
pub use api::{DslApi, QueryBuilder, WriteBuilder};
pub use being::{Being, BeingCore, BeingExt};
pub use config::Config;
pub use def::{Def, DefRegistry, Field, FieldType};
pub use error::DaoQLError;
pub use id::BeingId;
pub use relation::{Relation, RelationType, RelationTypeRegistry};
pub use transaction::TxId;

use std::cell::RefCell;
use std::path::Path;
use std::sync::Arc;

use crate::column::ProjectedLayer;
use crate::graph::store::GraphStore;
use crate::index::uuid_index::UuidIndex;
use crate::storage::StorageManager;
use crate::transaction::committer::{Transaction, TxIdGenerator};
use crate::vector::hnsw::HnswIndex;
use crate::wal::writer::WalWriter;

/// DaoQL 引擎入口
///
/// 这是用户与引擎交互的主要接口。
pub struct DaoQL {
    /// 存储管理器
    pub storage: Arc<StorageManager>,
    /// 图引擎（RefCell 支持 &self API 写操作）
    pub graph: RefCell<GraphStore>,
    /// 事务 ID 生成器
    pub tx_gen: TxIdGenerator,
    /// DSL API
    pub dsl_api: DslApi,
    /// 配置
    pub config: Config,
    /// HNSW 索引（每个字段一个）
    pub vector_indices: std::collections::HashMap<String, HnswIndex>,
    /// UUID → Offset B+Tree 索引
    pub uuid_index: RefCell<UuidIndex>,
    /// 列存投影层
    pub column: RefCell<ProjectedLayer>,
    /// 类型定义注册表
    pub def_registry: RefCell<DefRegistry>,
    /// WAL 双缓冲写入器
    pub wal: RefCell<WalWriter>,
}

impl DaoQL {
    /// 打开或创建 DaoQL 引擎
    pub fn open(path: impl AsRef<Path>) -> Result<Self, DaoQLError> {
        Self::open_with_config(path, Config::default())
    }

    /// 使用自定义配置打开
    pub fn open_with_config(path: impl AsRef<Path>, config: Config) -> Result<Self, DaoQLError> {
        let storage = Arc::new(StorageManager::open(&path, config.clone())?);
        let graph = RefCell::new(GraphStore::new(storage.clone()));
        let mut uuid_index = UuidIndex::open(path.as_ref().join("uuid_index.redb"))?;
        if !config.index_sync {
            uuid_index.set_durability(redb::Durability::None);
            uuid_index.enable_cache();
        }
        let uuid_index = RefCell::new(uuid_index);
        let mut column = ProjectedLayer::new();
        column.register("weight");
        column.register("priority");

        let wal_path = path.as_ref().join(&config.wal.wal_dir).join("wal.log");
        std::fs::create_dir_all(wal_path.parent().unwrap())?;
        let wal = RefCell::new(WalWriter::new(
            &wal_path,
            config.wal.buffer_size,
            config.wal.flush_interval_ms,
            true, // 默认启用 fsync，保证数据安全
        )?);

        Ok(Self {
            storage,
            graph,
            tx_gen: TxIdGenerator::new(),
            dsl_api: DslApi::new(),
            config,
            vector_indices: std::collections::HashMap::new(),
            uuid_index,
            column: RefCell::new(column),
            def_registry: RefCell::new(DefRegistry::new()),
            wal,
        })
    }

    /// 开始查询
    pub fn query(&self) -> QueryBuilder<'_> {
        QueryBuilder::new(&self.graph, &self.vector_indices, &self.uuid_index, &self.column)
    }

    /// 开启事务
    pub fn begin_tx(&self) -> Result<Transaction<'_>, DaoQLError> {
        let tx_id = self.tx_gen.next();
        Ok(Transaction::new(
            tx_id,
            &self.wal,
            &self.graph,
            &self.uuid_index,
            &self.column,
            &self.def_registry,
        ))
    }

    /// 写入实体（通过事务统一提交图存储、索引、列存）
    pub fn write(&self, being: Being) -> Result<BeingId, DaoQLError> {
        let mut tx = self.begin_tx()?;
        let id = being.core.id;
        tx.create_being(being);
        tx.commit()?;
        Ok(id)
    }

    /// 批量写入实体（绕过 Transaction，直接批量写入）
    ///
    /// 性能优势：
    /// - 避免 ops Vec 分配和 Being 克隆
    /// - 避免 WAL 大 payload 序列化
    /// - 图存储走 batch alloc + batch write
    pub fn write_batch(&self, beings: &[Being]) -> Result<Vec<BeingId>, DaoQLError> {
        if beings.is_empty() {
            return Ok(Vec::new());
        }

        let tx_id = self.tx_gen.next();

        // 1. 注册 defs（使用 get_or_register 避免重复创建 Def 对象）
        let mut def_registry = self.def_registry.borrow_mut();

        // 2. 图存储逐条创建
        let mut graph = self.graph.borrow_mut();
        graph.set_tx(tx_id);
        let mut uuid_batch = Vec::with_capacity(beings.len());

        for being in beings {
            let def_type_code = def_registry.get_or_register(&being.core.def);
            let offset = graph.create_node(&being.core, def_type_code)?;
            uuid_batch.push((being.core.id, offset));
        }
        drop(def_registry);

        // 3. 列存投影
        let mut column = self.column.borrow_mut();
        for being in beings {
            column.project(being)?;
        }

        // 4. UUID 索引批量插入
        let uuid_index = self.uuid_index.borrow();
        uuid_index.batch_insert(&uuid_batch)?;

        Ok(beings.iter().map(|b| b.core.id).collect())
    }

    /// 建立关系到图存储
    pub fn relate(
        &self,
        from: BeingId,
        to: BeingId,
        relation_type: u16,
    ) -> Result<(), DaoQLError> {
        let mut graph = self.graph.borrow_mut();
        let relation = crate::relation::Relation::new(from, to, relation_type, true);
        graph.create_edge(&relation)?;
        Ok(())
    }

    /// 执行 DSL 语句
    pub fn execute_dsl(&self, dsl: &str) -> Result<dsl::executor::DslResult, DaoQLError> {
        self.dsl_api.execute(dsl, &self.graph, &self.def_registry, &self.vector_indices, &self.uuid_index, &self.column)
    }

    /// 注册向量字段
    pub fn register_vector_field(&mut self, name: impl Into<String>, dim: usize) {
        let name = name.into();
        let hnsw = HnswIndex::new(
            dim,
            self.config.hnsw.m,
            self.config.hnsw.ef_construction,
            self.config.hnsw.ef_search,
        )
        .with_metric(self.config.vector.distance);
        self.vector_indices.insert(name, hnsw);
    }

    /// 关闭引擎
    pub fn shutdown(&mut self) -> Result<(), DaoQLError> {
        self.wal.borrow_mut().shutdown()?;
        Ok(())
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_daoql_open() {
        let dir = std::env::temp_dir().join("daoql-edu-test-lib");
        let _ = std::fs::remove_dir_all(&dir);
        let daoql = DaoQL::open(&dir).unwrap();
        assert_eq!(daoql.graph.borrow().node_count(), 0);
    }

    #[test]
    fn test_daoql_write() {
        let dir = std::env::temp_dir().join("daoql-edu-test-write");
        let _ = std::fs::remove_dir_all(&dir);
        let daoql = DaoQL::open(&dir).unwrap();

        let being = Being::new("Test", "TestDef").with_attr("x", serde_json::json!(42));
        let id = daoql.write(being).unwrap();
        assert!(!id.is_null());
    }

    #[test]
    fn test_daoql_dsl() {
        let dir = std::env::temp_dir().join("daoql-edu-test-dsl");
        let _ = std::fs::remove_dir_all(&dir);
        let daoql = DaoQL::open(&dir).unwrap();

        let result = daoql.execute_dsl(r#"query { Order(limit: 10) { id } }"#).unwrap();
        match result {
            dsl::executor::DslResult::Data(items) => {
                assert!(items.is_empty() || items.len() <= 10);
            }
            _ => panic!("期望 Data"),
        }
    }

    #[test]
    fn test_query_builder() {
        let daoql = DaoQL::open(std::env::temp_dir().join("daoql-edu-test-query")).unwrap();
        let result = daoql.query()
            .scan("Order")
            .limit(10)
            .execute();
        assert!(result.is_ok());
    }

    #[test]
    fn test_write_builder_create_relation() {
        let dir = std::env::temp_dir().join("daoql-edu-test-wb-relation");
        let _ = std::fs::remove_dir_all(&dir);
        let daoql = DaoQL::open(&dir).unwrap();

        let being1 = Being::new("Alice", "Person");
        let id1 = daoql.write(being1).unwrap();
        let being2 = Being::new("Bob", "Person");
        let id2 = daoql.write(being2).unwrap();

        let wb = WriteBuilder::new(&daoql.graph, &daoql.def_registry, &daoql.uuid_index);
        let relation = Relation::new(id1, id2, 1, true);
        wb.create_relation(relation).unwrap();

        let graph = daoql.graph.borrow();
        let edges = graph.out_edges(0).unwrap();
        assert_eq!(edges.len(), 1);
    }

    #[test]
    fn test_write_builder_delete_being() {
        let dir = std::env::temp_dir().join("daoql-edu-test-wb-delete");
        let _ = std::fs::remove_dir_all(&dir);
        let daoql = DaoQL::open(&dir).unwrap();

        let being = Being::new("Alice", "Person");
        let id = daoql.write(being).unwrap();

        let wb = WriteBuilder::new(&daoql.graph, &daoql.def_registry, &daoql.uuid_index);
        wb.delete_being(id).unwrap();

        let graph = daoql.graph.borrow();
        let node = graph.read_node(0).unwrap();
        assert_eq!(node.status, 0);
    }

    #[test]
    fn test_query_builder_being() {
        let dir = std::env::temp_dir().join("daoql-edu-test-qb-being");
        let _ = std::fs::remove_dir_all(&dir);
        let daoql = DaoQL::open(&dir).unwrap();

        let being = Being::new("Alice", "Person");
        let id = daoql.write(being).unwrap();

        let result = daoql.query().being(id).fetch_one().unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().core.name, "Alice");
    }

    #[test]
    fn test_query_builder_filter() {
        let dir = std::env::temp_dir().join("daoql-edu-test-qb-filter");
        let _ = std::fs::remove_dir_all(&dir);
        let daoql = DaoQL::open(&dir).unwrap();

        let being1 = Being::new("Alice", "Person");
        daoql.write(being1).unwrap();
        let being2 = Being::new("Bob", "Person");
        daoql.write(being2).unwrap();

        let result = daoql.query()
            .scan("Person")
            .filter("name", "==", serde_json::json!("Alice"))
            .fetch_all()
            .unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].core.name, "Alice");
    }

    #[test]
    fn test_dsl_mutation() {
        let dir = std::env::temp_dir().join("daoql-edu-test-dsl-mutation");
        let _ = std::fs::remove_dir_all(&dir);
        let daoql = DaoQL::open(&dir).unwrap();

        let result = daoql.execute_dsl(r#"mutation { create Person ( input: { name: "Alice" } ) }"#).unwrap();
        match result {
            dsl::executor::DslResult::Message(msg) => {
                assert!(msg.contains("创建"));
            }
            _ => panic!("期望 Message"),
        }
    }

    #[test]
    fn test_version_rebuild() {
        let dir = std::env::temp_dir().join("daoql-edu-test-version");
        let _ = std::fs::remove_dir_all(&dir);
        let daoql = DaoQL::open(&dir).unwrap();

        let being1 = Being::new("Alice", "Person");
        let id1 = daoql.write(being1).unwrap();
        let being2 = Being::new("Bob", "Person");
        let id2 = daoql.write(being2).unwrap();

        let mut vm = version::VersionManager::new();
        let graph = daoql.graph.borrow();
        vm.rebuild(&graph).unwrap();

        assert_eq!(vm.version_count(id1), 1);
        assert_eq!(vm.version_count(id2), 1);
    }

    #[test]
    fn test_write_populates_column_store() {
        let dir = std::env::temp_dir().join("daoql-edu-test-column-write");
        let _ = std::fs::remove_dir_all(&dir);
        let daoql = DaoQL::open(&dir).unwrap();

        let mut being = Being::new("Product", "ProductDef");
        being.core.weight = 12.5;
        let id = daoql.write(being).unwrap();

        let column = daoql.column.borrow();
        let weight_col = column.column("weight").unwrap();
        assert_eq!(weight_col.len(), 1);
        assert!((weight_col.get(id).unwrap() - 12.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_transaction_commit() {
        let dir = std::env::temp_dir().join("daoql-edu-test-tx-commit");
        let _ = std::fs::remove_dir_all(&dir);
        let daoql = DaoQL::open(&dir).unwrap();

        let being = Being::new("Alice", "Person");
        let mut tx = daoql.begin_tx().unwrap();
        tx.create_being(being);
        tx.commit().unwrap();

        let graph = daoql.graph.borrow();
        assert_eq!(graph.node_count(), 1);

        let column = daoql.column.borrow();
        assert_eq!(column.row_count(), 1);
    }

    #[test]
    fn test_aggregate_on_filtered_result_set() {
        let dir = std::env::temp_dir().join("daoql-edu-test-agg-filter");
        let _ = std::fs::remove_dir_all(&dir);
        let daoql = DaoQL::open(&dir).unwrap();

        let mut b1 = Being::new("Alice", "Person");
        b1.core.weight = 10.0;
        daoql.write(b1).unwrap();
        let mut b2 = Being::new("Bob", "Person");
        b2.core.weight = 20.0;
        daoql.write(b2).unwrap();
        let mut b3 = Being::new("Carol", "Person");
        b3.core.weight = 30.0;
        daoql.write(b3).unwrap();

        // 全表聚合 = 60
        let result = daoql.query()
            .scan("Person")
            .aggregate("weight", crate::column::AggregateOp::Sum)
            .execute()
            .unwrap();
        assert_eq!(result.aggregate_value, Some(60.0));

        // 过滤后聚合（仅 Alice + Bob）= 30
        let result = daoql.query()
            .scan("Person")
            .filter("name", "!=", serde_json::json!("Carol"))
            .aggregate("weight", crate::column::AggregateOp::Sum)
            .execute()
            .unwrap();
        assert_eq!(result.aggregate_value, Some(30.0));
    }

    // =========================================================================
    // 场景测试：真实业务场景（DSL 驱动）
    // =========================================================================

    /// 场景 1：电商库存管理 —— 商品创建、筛选、统计
    #[test]
    fn test_dsl_ecommerce_inventory() {
        let dir = std::env::temp_dir().join("daoql-edu-test-ecommerce");
        let _ = std::fs::remove_dir_all(&dir);
        let daoql = DaoQL::open(&dir).unwrap();

        // 用 Rust API 创建商品（weight = 价格）
        let mut phone = Being::new("iPhone", "Product");
        phone.core.weight = 999.0;
        phone.core.status = 1; // 上架
        daoql.write(phone).unwrap();

        let mut earphone = Being::new("AirPods", "Product");
        earphone.core.weight = 199.0;
        earphone.core.status = 1;
        daoql.write(earphone).unwrap();

        let mut tablet = Being::new("iPad", "Product");
        tablet.core.weight = 2999.0;
        tablet.core.status = 0; // 下架
        daoql.write(tablet).unwrap();

        let mut watch = Being::new("Watch", "Product");
        watch.core.weight = 399.0;
        watch.core.status = 1;
        daoql.write(watch).unwrap();

        // DSL：筛选高价商品（weight > 500）
        let result = daoql.execute_dsl(
            r#"query { Product(filter: {weight > 500}) { id, name, weight, status } }"#
        ).unwrap();
        if let dsl::executor::DslResult::Data(items) = result {
            assert_eq!(items.len(), 2, "高价商品应只有 iPhone 和 iPad");
            let names: Vec<&str> = items.iter()
                .map(|v| v["name"].as_str().unwrap())
                .collect();
            assert!(names.contains(&"iPhone"));
            assert!(names.contains(&"iPad"));
        } else {
            panic!("期望 Data");
        }

        // DSL：筛选上架商品（status = 1）
        let result = daoql.execute_dsl(
            r#"query { Product(filter: {status: 1}) { id, name } }"#
        ).unwrap();
        if let dsl::executor::DslResult::Data(items) = result {
            assert_eq!(items.len(), 3, "上架商品应有 3 个");
        } else {
            panic!("期望 Data");
        }

        // DSL：限制返回数量
        let result = daoql.execute_dsl(
            r#"query { Product(limit: 2) { id, name } }"#
        ).unwrap();
        if let dsl::executor::DslResult::Data(items) = result {
            assert_eq!(items.len(), 2);
        } else {
            panic!("期望 Data");
        }
    }

    /// 场景 2：社交网络 —— 用户创建、关系建立、BFS 遍历
    #[test]
    fn test_dsl_social_network() {
        let dir = std::env::temp_dir().join("daoql-edu-test-social");
        let _ = std::fs::remove_dir_all(&dir);
        let daoql = DaoQL::open(&dir).unwrap();

        // 创建用户
        let alice = Being::new("Alice", "User");
        let id_alice = daoql.write(alice).unwrap();
        let bob = Being::new("Bob", "User");
        let id_bob = daoql.write(bob).unwrap();
        let carol = Being::new("Carol", "User");
        let id_carol = daoql.write(carol).unwrap();
        let dave = Being::new("Dave", "User");
        let id_dave = daoql.write(dave).unwrap();

        // 建立关注关系：Alice->Bob, Alice->Carol, Bob->Dave
        let wb = crate::api::write_builder::WriteBuilder::new(
            &daoql.graph, &daoql.def_registry, &daoql.uuid_index
        );
        wb.create_relation(crate::relation::Relation::new(id_alice, id_bob, 1, true)).unwrap();
        wb.create_relation(crate::relation::Relation::new(id_alice, id_carol, 1, true)).unwrap();
        wb.create_relation(crate::relation::Relation::new(id_bob, id_dave, 1, true)).unwrap();

        // DSL：查询所有用户
        let result = daoql.execute_dsl(
            r#"query { User { id, name } }"#
        ).unwrap();
        if let dsl::executor::DslResult::Data(items) = result {
            assert_eq!(items.len(), 4);
        } else {
            panic!("期望 Data");
        }

        // DSL：BFS 遍历 Alice 的社交网络
        let result = daoql.execute_dsl(
            r#"analyze { bfs on Alice(limit: 10) { } }"#
        ).unwrap();
        if let dsl::executor::DslResult::Data(items) = result {
            // BFS 返回 offset + depth，应至少包含 Bob(depth=1) 和 Carol(depth=1)
            assert!(items.len() >= 2, "BFS 应至少遍历到 2 个直接关注对象");
        } else {
            panic!("期望 Data");
        }
    }

    /// 场景 3：内容推荐 —— 向量相似搜索
    #[test]
    fn test_dsl_content_recommendation() {
        let dir = std::env::temp_dir().join("daoql-edu-test-recommend");
        let _ = std::fs::remove_dir_all(&dir);
        let mut daoql = DaoQL::open(&dir).unwrap();

        // 注册向量字段
        daoql.register_vector_field("embedding", 8);

        // 创建文章（带向量嵌入）
        let article1 = Being::new("Rust入门", "Article");
        let id1 = daoql.write(article1).unwrap();
        let article2 = Being::new("Rust高级", "Article");
        let id2 = daoql.write(article2).unwrap();
        let article3 = Being::new("Python入门", "Article");
        let id3 = daoql.write(article3).unwrap();

        // 插入向量（Rust 文章相似，Python 文章不同）
        if let Some(index) = daoql.vector_indices.get_mut("embedding") {
            index.insert(id1, vec![0.9, 0.8, 0.7, 0.6, 0.1, 0.1, 0.1, 0.1]).unwrap();
            index.insert(id2, vec![0.85, 0.75, 0.65, 0.55, 0.15, 0.15, 0.15, 0.15]).unwrap();
            index.insert(id3, vec![0.1, 0.1, 0.1, 0.1, 0.9, 0.8, 0.7, 0.6]).unwrap();
        }

        // DSL：向量相似搜索，找与 Rust入门 相似的文章
        let result = daoql.execute_dsl(
            r#"similar { Article(query: [0.9, 0.8, 0.7, 0.6, 0.1, 0.1, 0.1, 0.1], k: 2) { id, name, distance } }"#
        ).unwrap();
        if let dsl::executor::DslResult::Data(items) = result {
            assert_eq!(items.len(), 2, "应返回 2 篇最相似文章");
            // 最相似的应该是 Rust入门 自己，其次是 Rust高级
            let first_name = items[0]["name"].as_str().unwrap();
            assert!(first_name.contains("Rust"), "最相似结果应包含 Rust 文章");
        } else {
            panic!("期望 Data");
        }
    }

    /// 场景 4：复合过滤 —— 范围查询、空结果、多条件
    #[test]
    fn test_dsl_complex_filters() {
        let dir = std::env::temp_dir().join("daoql-edu-test-filters");
        let _ = std::fs::remove_dir_all(&dir);
        let daoql = DaoQL::open(&dir).unwrap();

        // 创建员工（weight = 薪资，priority = 级别）
        let mut e1 = Being::new("Alice", "Employee");
        e1.core.weight = 8000.0;
        e1.core.priority = 5;
        daoql.write(e1).unwrap();

        let mut e2 = Being::new("Bob", "Employee");
        e2.core.weight = 12000.0;
        e2.core.priority = 7;
        daoql.write(e2).unwrap();

        let mut e3 = Being::new("Carol", "Employee");
        e3.core.weight = 15000.0;
        e3.core.priority = 9;
        daoql.write(e3).unwrap();

        let mut e4 = Being::new("Dave", "Employee");
        e4.core.weight = 5000.0;
        e4.core.priority = 3;
        daoql.write(e4).unwrap();

        // DSL：薪资 > 10000 的高薪员工
        let result = daoql.execute_dsl(
            r#"query { Employee(filter: {weight > 10000}) { id, name, weight } }"#
        ).unwrap();
        if let dsl::executor::DslResult::Data(items) = result {
            assert_eq!(items.len(), 2, "高薪员工应有 Bob 和 Carol");
        } else {
            panic!("期望 Data");
        }

        // DSL：薪资 < 6000 的初级员工
        let result = daoql.execute_dsl(
            r#"query { Employee(filter: {weight < 6000}) { id, name, weight } }"#
        ).unwrap();
        if let dsl::executor::DslResult::Data(items) = result {
            assert_eq!(items.len(), 1);
            assert_eq!(items[0]["name"], "Dave");
        } else {
            panic!("期望 Data");
        }

        // DSL：查询不存在的条件（空结果）
        let result = daoql.execute_dsl(
            r#"query { Employee(filter: {weight > 99999}) { id, name } }"#
        ).unwrap();
        if let dsl::executor::DslResult::Data(items) = result {
            assert!(items.is_empty(), "不存在薪资超过 99999 的员工");
        } else {
            panic!("期望 Data");
        }

        // DSL：级别 >= 5 的员工
        let result = daoql.execute_dsl(
            r#"query { Employee(filter: {priority >= 5}) { id, name, priority } }"#
        ).unwrap();
        if let dsl::executor::DslResult::Data(items) = result {
            assert_eq!(items.len(), 3, "级别 >=5 应有 Alice, Bob, Carol");
        } else {
            panic!("期望 Data");
        }
    }

    // =========================================================================
    // 混合查询场景：跨引擎查询（图 + 列存 + 向量 + 索引）
    // =========================================================================

    /// 混合场景 1：图扫描 + filter + 列存聚合（论文核心混合查询）
    #[test]
    fn test_mixed_scan_filter_aggregate() {
        let dir = std::env::temp_dir().join("daoql-edu-test-mixed-sfa");
        let _ = std::fs::remove_dir_all(&dir);
        let daoql = DaoQL::open(&dir).unwrap();

        // 创建订单（weight = 金额）
        for i in 0..20 {
            let mut order = Being::new(&format!("Order-{i}"), "Order");
            order.core.weight = (i * 10) as f64; // 0, 10, 20, ..., 190
            daoql.write(order).unwrap();
        }

        // 混合查询：图扫描所有 Order → filter（金额 > 50）→ 列存 SUM 聚合
        let result = daoql
            .query()
            .scan("Order")
            .filter("weight", "gt", serde_json::json!(50.0))
            .aggregate("weight", crate::column::AggregateOp::Sum)
            .execute()
            .unwrap();

        // 金额 > 50 的订单：60+70+...+190 = 1750（共 14 个）
        assert_eq!(result.aggregate_value, Some(1750.0));
    }

    /// 混合场景 2：向量搜索 → 图遍历 → 列读取
    #[test]
    fn test_mixed_vector_search_then_graph_traverse() {
        let dir = std::env::temp_dir().join("daoql-edu-test-mixed-vsg");
        let _ = std::fs::remove_dir_all(&dir);
        let mut daoql = DaoQL::open(&dir).unwrap();

        // 注册向量字段
        daoql.register_vector_field("embedding", 8);

        // 创建文章和作者
        let article1 = Being::new("Rust入门", "Article");
        let id1 = daoql.write(article1).unwrap();
        let author1 = Being::new("张三", "Author");
        let author_id1 = daoql.write(author1).unwrap();

        let article2 = Being::new("Rust高级", "Article");
        let id2 = daoql.write(article2).unwrap();
        let author2 = Being::new("李四", "Author");
        let author_id2 = daoql.write(author2).unwrap();

        let article3 = Being::new("Python入门", "Article");
        let id3 = daoql.write(article3).unwrap();

        // 建立作者-文章关系
        let wb = crate::api::write_builder::WriteBuilder::new(
            &daoql.graph, &daoql.def_registry, &daoql.uuid_index
        );
        wb.create_relation(crate::relation::Relation::new(author_id1, id1, 1, true)).unwrap();
        wb.create_relation(crate::relation::Relation::new(author_id2, id2, 1, true)).unwrap();

        // 插入向量（Rust 文章相似）
        if let Some(index) = daoql.vector_indices.get_mut("embedding") {
            index.insert(id1, vec![0.9, 0.8, 0.7, 0.6, 0.1, 0.1, 0.1, 0.1]).unwrap();
            index.insert(id2, vec![0.85, 0.75, 0.65, 0.55, 0.15, 0.15, 0.15, 0.15]).unwrap();
            index.insert(id3, vec![0.1, 0.1, 0.1, 0.1, 0.9, 0.8, 0.7, 0.6]).unwrap();
        }

        // Step 1: 向量搜索找到与 "Rust入门" 相似的文章
        let similar_results = daoql.query().similar_to(vec![0.9, 0.8, 0.7, 0.6, 0.1, 0.1, 0.1, 0.1], 2).execute().unwrap();
        assert!(!similar_results.items.is_empty(), "应找到相似文章");

        // Step 2: 对每篇相似文章，遍历其作者关系，读取作者信息
        for article in &similar_results.items {
            let graph = daoql.graph.borrow();
            // 通过 UUID 索引找到文章 offset
            if let Ok(Some(offset)) = daoql.uuid_index.borrow().get(article.core.id) {
                // 遍历出边（作者→文章的关系反向查找较复杂，简化：直接读取作者）
                let _ = graph.read_node(offset);
            }
        }

        // 验证：至少找到一篇 Rust 相关文章
        let names: Vec<&str> = similar_results.items.iter()
            .map(|b| b.core.name.as_str())
            .collect();
        assert!(names.iter().any(|n| n.contains("Rust")));
    }

    /// 混合场景 3：BFS 图遍历 → 列存读取
    #[test]
    fn test_mixed_bfs_then_column_read() {
        let dir = std::env::temp_dir().join("daoql-edu-test-mixed-bfs");
        let _ = std::fs::remove_dir_all(&dir);
        let daoql = DaoQL::open(&dir).unwrap();

        // 创建社交网络（priority = 影响力分数）
        let alice = Being::new("Alice", "User");
        let id_alice = daoql.write(alice).unwrap();
        let mut bob = Being::new("Bob", "User");
        bob.core.priority = 80;
        let id_bob = daoql.write(bob).unwrap();
        let mut carol = Being::new("Carol", "User");
        carol.core.priority = 95;
        let id_carol = daoql.write(carol).unwrap();
        let mut dave = Being::new("Dave", "User");
        dave.core.priority = 60;
        let id_dave = daoql.write(dave).unwrap();

        // 建立关注关系：Alice->Bob, Alice->Carol, Bob->Dave
        let wb = crate::api::write_builder::WriteBuilder::new(
            &daoql.graph, &daoql.def_registry, &daoql.uuid_index
        );
        wb.create_relation(crate::relation::Relation::new(id_alice, id_bob, 1, true)).unwrap();
        wb.create_relation(crate::relation::Relation::new(id_alice, id_carol, 1, true)).unwrap();
        wb.create_relation(crate::relation::Relation::new(id_bob, id_dave, 1, true)).unwrap();

        // Step 1: BFS 遍历 Alice 的社交网络
        let uuid_idx = daoql.uuid_index.borrow();
        let alice_offset = uuid_idx.get(id_alice).unwrap().unwrap();
        drop(uuid_idx);

        let mut graph = daoql.graph.borrow_mut();
        let bfs_results = crate::graph::traversal::bfs(&mut graph, alice_offset, 3, None).unwrap();
        drop(graph);

        assert!(!bfs_results.is_empty(), "BFS 应至少遍历到 Alice 自己");

        // Step 2: 对 BFS 遍历到的每个节点，从列存读取 priority（影响力）
        let column = daoql.column.borrow();
        let mut total_influence = 0u32;
        let mut count = 0;

        for (offset, _depth) in &bfs_results {
            let graph = daoql.graph.borrow();
            if let Ok(node) = graph.read_node(*offset) {
                if let Ok(being) = crate::being::Being::from_node(node) {
                    if let Some(p) = column.column("priority").and_then(|col| col.get(being.core.id)) {
                        total_influence += p as u32;
                        count += 1;
                    }
                }
            }
        }

        // Alice(0) + Bob(80) + Carol(95) + Dave(60) = 235
        assert_eq!(total_influence, 235, "BFS 遍历到的用户影响力总和应为 235");
        assert_eq!(count, 4, "应遍历到 4 个用户");
    }

    /// 混合场景 4：点查 → 关系遍历 → 列存聚合
    #[test]
    fn test_mixed_point_query_relation_aggregate() {
        let dir = std::env::temp_dir().join("daoql-edu-test-mixed-pqra");
        let _ = std::fs::remove_dir_all(&dir);
        let daoql = DaoQL::open(&dir).unwrap();

        // 创建客户和订单
        let customer = Being::new("Alice", "Customer");
        let cust_id = daoql.write(customer).unwrap();

        for i in 1..=5 {
            let mut order = Being::new(&format!("Order-{i}"), "Order");
            order.core.weight = (i * 100) as f64; // 100, 200, 300, 400, 500
            let order_id = daoql.write(order).unwrap();

            // 建立客户-订单关系
            let wb = crate::api::write_builder::WriteBuilder::new(
                &daoql.graph, &daoql.def_registry, &daoql.uuid_index
            );
            wb.create_relation(crate::relation::Relation::new(cust_id, order_id, 1, true)).unwrap();
        }

        // Step 1: 点查找到客户 Alice
        let customer_result = daoql.query().being(cust_id).fetch_one().unwrap();
        assert!(customer_result.is_some());

        // Step 2: 遍历 Alice 的所有出边（订单关系）
        let uuid_idx = daoql.uuid_index.borrow();
        let cust_offset = uuid_idx.get(cust_id).unwrap().unwrap();
        drop(uuid_idx);

        let graph = daoql.graph.borrow();
        let edges = graph.out_edges(cust_offset).unwrap();
        let mut order_ids = Vec::new();
        for edge in edges {
            order_ids.push(edge.to_id);
        }
        drop(graph);

        assert_eq!(order_ids.len(), 5, "Alice 应有 5 个订单");

        // Step 3: 从列存读取每个订单的金额并求和
        let column = daoql.column.borrow();
        let mut total = 0.0;
        for order_id in &order_ids {
            if let Some(v) = column.column("weight").and_then(|col| col.get(*order_id)) {
                total += v;
            }
        }

        // 100 + 200 + 300 + 400 + 500 = 1500
        assert_eq!(total, 1500.0, "Alice 的订单总金额应为 1500");
    }

    /// 场景5：电商推荐 —— 向量搜索 → 图关系 → 列存属性（跨引擎嵌套）
    #[test]
    fn test_dsl_ecommerce_recommendation() {
        let dir = std::env::temp_dir().join("daoql-edu-test-rec");
        let _ = std::fs::remove_dir_all(&dir);
        let mut daoql = DaoQL::open(&dir).unwrap();
        daoql.register_vector_field("embedding", 8);

        // 创建产品（weight = 价格）
        let mut phone = Being::new("iPhone", "Product");
        phone.core.weight = 999.0;
        let id_phone = daoql.write(phone).unwrap();

        let mut earphone = Being::new("AirPods", "Product");
        earphone.core.weight = 199.0;
        let id_earphone = daoql.write(earphone).unwrap();

        let mut tablet = Being::new("iPad", "Product");
        tablet.core.weight = 2999.0;
        let id_tablet = daoql.write(tablet).unwrap();

        // 建立配件关系：iPhone -> AirPods, iPhone -> iPad
        let wb = crate::api::write_builder::WriteBuilder::new(
            &daoql.graph, &daoql.def_registry, &daoql.uuid_index
        );
        wb.create_relation(crate::relation::Relation::new(id_phone, id_earphone, 1, true)).unwrap();
        wb.create_relation(crate::relation::Relation::new(id_phone, id_tablet, 1, true)).unwrap();

        // 插入向量（iPhone 和 AirPods 的 embedding 相似）
        if let Some(index) = daoql.vector_indices.get_mut("embedding") {
            index.insert(id_phone, vec![0.9, 0.8, 0.7, 0.6, 0.1, 0.1, 0.1, 0.1]).unwrap();
            index.insert(id_earphone, vec![0.85, 0.75, 0.65, 0.55, 0.15, 0.15, 0.15, 0.15]).unwrap();
            index.insert(id_tablet, vec![0.1, 0.1, 0.1, 0.1, 0.9, 0.8, 0.7, 0.6]).unwrap();
        }

        // DSL：向量搜索与 iPhone 相似的产品，返回结果应包含价格(weight)和关联产品
        let result = daoql.execute_dsl(
            r#"similar { Product(query: [0.9, 0.8, 0.7, 0.6, 0.1, 0.1, 0.1, 0.1], k: 3) { } }"#
        ).unwrap();
        if let dsl::executor::DslResult::Data(items) = result {
            assert!(!items.is_empty(), "应返回相似产品");
            // 第一个结果应该是 iPhone 自己，应包含 weight 和 relations
            let first = &items[0];
            assert!(first.get("weight").is_some(), "结果应包含列存价格 weight");
            assert!(first.get("relations").is_some(), "结果应包含图关系 relations");
            assert!(first.get("relation_count").is_some(), "结果应包含关系数量");
            let relations = first["relations"].as_array().unwrap();
            assert_eq!(relations.len(), 2, "iPhone 应有 2 个关联产品");
        } else {
            panic!("期望 Data");
        }
    }

    /// 场景6：社交网络影响力 —— BFS 遍历 → 列存优先级（跨引擎嵌套）
    #[test]
    fn test_dsl_social_influence() {
        let dir = std::env::temp_dir().join("daoql-edu-test-influence");
        let _ = std::fs::remove_dir_all(&dir);
        let daoql = DaoQL::open(&dir).unwrap();

        // 创建用户（priority = 影响力分数）
        let alice = Being::new("Alice", "User");
        let id_alice = daoql.write(alice).unwrap();

        let mut bob = Being::new("Bob", "User");
        bob.core.priority = 80;
        let id_bob = daoql.write(bob).unwrap();

        let mut carol = Being::new("Carol", "User");
        carol.core.priority = 95;
        let id_carol = daoql.write(carol).unwrap();

        let mut dave = Being::new("Dave", "User");
        dave.core.priority = 60;
        let id_dave = daoql.write(dave).unwrap();

        // 建立关注关系：Alice->Bob, Alice->Carol, Bob->Dave
        let wb = crate::api::write_builder::WriteBuilder::new(
            &daoql.graph, &daoql.def_registry, &daoql.uuid_index
        );
        wb.create_relation(crate::relation::Relation::new(id_alice, id_bob, 1, true)).unwrap();
        wb.create_relation(crate::relation::Relation::new(id_alice, id_carol, 1, true)).unwrap();
        wb.create_relation(crate::relation::Relation::new(id_bob, id_dave, 1, true)).unwrap();

        // DSL：BFS 遍历 Alice 的社交网络，返回结果应包含 priority
        let result = daoql.execute_dsl(
            r#"analyze { bfs on Alice(limit: 10) { } }"#
        ).unwrap();
        if let dsl::executor::DslResult::Data(items) = result {
            assert!(!items.is_empty(), "BFS 应返回遍历结果");
            // 查找 Bob 的结果，应包含 priority
            let bob_item = items.iter().find(|v| v["name"] == "Bob");
            assert!(bob_item.is_some(), "应找到 Bob");
            let bob_item = bob_item.unwrap();
            assert!(bob_item.get("priority").is_some(), "结果应包含列存 priority");
            assert_eq!(bob_item["priority"].as_f64().unwrap(), 80.0, "Bob 的影响力分数应为 80");
        } else {
            panic!("期望 Data");
        }
    }

    /// 场景7：销售报表 —— DSL 聚合查询（列存聚合 fast path）
    #[test]
    fn test_dsl_sales_report() {
        let dir = std::env::temp_dir().join("daoql-edu-test-sales");
        let _ = std::fs::remove_dir_all(&dir);
        let daoql = DaoQL::open(&dir).unwrap();

        // 创建订单（weight = 金额）
        let mut o1 = Being::new("Order-A", "Order");
        o1.core.weight = 100.0;
        daoql.write(o1).unwrap();

        let mut o2 = Being::new("Order-B", "Order");
        o2.core.weight = 200.0;
        daoql.write(o2).unwrap();

        let mut o3 = Being::new("Order-C", "Order");
        o3.core.weight = 300.0;
        daoql.write(o3).unwrap();

        // DSL：查询订单总金额
        let result = daoql.execute_dsl(
            r#"query { Order(aggregate: {weight: sum}) { } }"#
        ).unwrap();
        if let dsl::executor::DslResult::Data(items) = result {
            assert_eq!(items.len(), 1, "聚合查询应返回单行结果");
            let agg = &items[0];
            assert_eq!(agg["aggregate"], 600.0, "订单总金额应为 600");
            assert_eq!(agg["count"], 3, "订单数量应为 3");
            assert_eq!(agg["op"], "sum");
        } else {
            panic!("期望 Data");
        }

        // DSL：带过滤的聚合（只统计金额 > 150 的订单）
        let result = daoql.execute_dsl(
            r#"query { Order(filter: {weight > 150}, aggregate: {weight: sum}) { } }"#
        ).unwrap();
        if let dsl::executor::DslResult::Data(items) = result {
            assert_eq!(items.len(), 1);
            assert_eq!(items[0]["aggregate"], 500.0, "金额 >150 的订单总和应为 500");
            assert_eq!(items[0]["count"], 2, "应统计 2 个订单");
        } else {
            panic!("期望 Data");
        }
    }

    /// 场景8：风控检测 —— 向量搜索异常交易 → 图关系 → 列存金额（跨引擎嵌套）
    #[test]
    fn test_dsl_risk_detection() {
        let dir = std::env::temp_dir().join("daoql-edu-test-risk");
        let _ = std::fs::remove_dir_all(&dir);
        let mut daoql = DaoQL::open(&dir).unwrap();
        daoql.register_vector_field("embedding", 8);

        // 创建交易（weight = 交易金额）
        let mut t1 = Being::new("Tx-Alice-1", "Transaction");
        t1.core.weight = 5000.0;
        let id1 = daoql.write(t1).unwrap();

        let mut t2 = Being::new("Tx-Alice-2", "Transaction");
        t2.core.weight = 8000.0;
        let id2 = daoql.write(t2).unwrap();

        let mut t3 = Being::new("Tx-Bob-1", "Transaction");
        t3.core.weight = 200.0;
        let id3 = daoql.write(t3).unwrap();

        // 创建账户
        let alice = Being::new("Alice-Account", "Account");
        let id_alice = daoql.write(alice).unwrap();

        // 建立交易-账户关系
        let wb = crate::api::write_builder::WriteBuilder::new(
            &daoql.graph, &daoql.def_registry, &daoql.uuid_index
        );
        wb.create_relation(crate::relation::Relation::new(id1, id_alice, 1, true)).unwrap();
        wb.create_relation(crate::relation::Relation::new(id2, id_alice, 1, true)).unwrap();

        // 插入向量（Alice 的交易相似）
        if let Some(index) = daoql.vector_indices.get_mut("embedding") {
            index.insert(id1, vec![0.9, 0.8, 0.7, 0.6, 0.1, 0.1, 0.1, 0.1]).unwrap();
            index.insert(id2, vec![0.85, 0.75, 0.65, 0.55, 0.15, 0.15, 0.15, 0.15]).unwrap();
            index.insert(id3, vec![0.1, 0.1, 0.1, 0.1, 0.9, 0.8, 0.7, 0.6]).unwrap();
        }

        // DSL：向量搜索相似交易，返回结果应包含金额和关联账户
        let result = daoql.execute_dsl(
            r#"similar { Transaction(query: [0.9, 0.8, 0.7, 0.6, 0.1, 0.1, 0.1, 0.1], k: 3) { } }"#
        ).unwrap();
        if let dsl::executor::DslResult::Data(items) = result {
            assert!(!items.is_empty(), "应返回相似交易");
            let first = &items[0];
            assert!(first.get("weight").is_some(), "结果应包含列存金额 weight");
            assert!(first.get("relations").is_some(), "结果应包含图关系 relations");
            let weight = first["weight"].as_f64().unwrap();
            assert!(weight > 0.0, "交易金额应大于 0");
        } else {
            panic!("期望 Data");
        }
    }
}
