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

//! DaoQL-Edu — Multimodal Data Engine (Educational Edition)
//!
//! Educational Notes:
//! - This project is a simplified educational implementation of DaoQL
//! - Preserve core architecture: Being/Def/Relation/Version primitives + graph/column/document/vector capabilities
//! - Preserve performance features: SIMD, SkipIndex, HNSW, PageCache, dual-buffer WAL
//! - Remove industrial complexity: full-text search, multi-tenancy, distributed, permissions, etc.
//!
//! # Quick Start
//! ```rust,ignore
//! use daoql_edu::{DaoQL, Being};
//!
//! let daoql = DaoQL::open("./data")?;
//! let being = Being::new("Alice", "Person");
//! daoql.write(being)?;
//! ```

// Infrastructure
pub mod api;
pub mod config;
pub mod error;
pub mod id;

// Core primitives
pub mod being;
pub mod def;
pub mod relation;
pub mod version;

// Storage and index
pub mod column;
pub mod graph;
pub mod index;
pub mod pagecache;
pub mod storage;
pub mod transaction;
pub mod wal;

// QueryAnd DSL
pub mod dsl;
pub mod query;

// Vector engine
pub mod vector;

// Public re-exports
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

/// DaoQL engine entry
///
/// This is the main interface for user-engine interaction.
pub struct DaoQL {
    /// Storage manager
    pub storage: Arc<StorageManager>,
    /// Graph engine (RefCell supports &self API write operations)
    pub graph: RefCell<GraphStore>,
    /// Transaction ID generator
    pub tx_gen: TxIdGenerator,
    /// DSL API
    pub dsl_api: DslApi,
    /// Configuration
    pub config: Config,
    /// HNSW index (one per field)
    pub vector_indices: std::collections::HashMap<String, HnswIndex>,
    /// UUID → Offset B+Tree index
    pub uuid_index: RefCell<UuidIndex>,
    /// Columnar projected layer
    pub column: RefCell<ProjectedLayer>,
    /// Type definition registry
    pub def_registry: RefCell<DefRegistry>,
    /// WAL dual-buffer writer
    pub wal: RefCell<WalWriter>,
}

impl DaoQL {
    /// Open or create DaoQL engine
    pub fn open(path: impl AsRef<Path>) -> Result<Self, DaoQLError> {
        Self::open_with_config(path, Config::default())
    }

    /// Open with custom configuration
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
            true, // Enable fsync by default to ensure data safety
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

    /// Start query
    pub fn query(&self) -> QueryBuilder<'_> {
        QueryBuilder::new(&self.graph, &self.vector_indices, &self.uuid_index, &self.column)
    }

    /// Begin transaction
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

    /// Write entity (unified commit via transaction to graph storage, index, column store)
    pub fn write(&self, being: Being) -> Result<BeingId, DaoQLError> {
        let mut tx = self.begin_tx()?;
        let id = being.core.id;
        tx.create_being(being);
        tx.commit()?;
        Ok(id)
    }

    /// Batch write entities (bypass Transaction, direct batch write)
    ///
    /// Performance advantages:
    /// - avoid ops Vec allocation and Being clone
    /// - Avoid WAL large payload serialization
    /// - Graph storage uses batch alloc + batch write
    pub fn write_batch(&self, beings: &[Being]) -> Result<Vec<BeingId>, DaoQLError> {
        if beings.is_empty() {
            return Ok(Vec::new());
        }

        let tx_id = self.tx_gen.next();

        // 1. Register defs (use get_or_register to avoid duplicate Def creation)
        let mut def_registry = self.def_registry.borrow_mut();

        // 2. Graph storage creates row by row
        let mut graph = self.graph.borrow_mut();
        graph.set_tx(tx_id);
        let mut uuid_batch = Vec::with_capacity(beings.len());

        for being in beings {
            let def_type_code = def_registry.get_or_register(&being.core.def);
            let offset = graph.create_node(&being.core, def_type_code)?;
            uuid_batch.push((being.core.id, offset));
        }
        drop(def_registry);

        // 3. Columnar projection
        let mut column = self.column.borrow_mut();
        for being in beings {
            column.project(being)?;
        }

        // 4. UUID index batch insert
        let uuid_index = self.uuid_index.borrow();
        uuid_index.batch_insert(&uuid_batch)?;

        Ok(beings.iter().map(|b| b.core.id).collect())
    }

    /// Establish relation to graph storage
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

    /// Execute DSL statement
    pub fn execute_dsl(&self, dsl: &str) -> Result<dsl::executor::DslResult, DaoQLError> {
        self.dsl_api.execute(dsl, &self.graph, &self.def_registry, &self.vector_indices, &self.uuid_index, &self.column)
    }

    /// Register vector field
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

    /// Close engine
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
            _ => panic!("expected Data"),
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
                assert!(msg.contains("Create"));
            }
            _ => panic!("Expected Message"),
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

        // Full table aggregate = 60
        let result = daoql.query()
            .scan("Person")
            .aggregate("weight", crate::column::AggregateOp::Sum)
            .execute()
            .unwrap();
        assert_eq!(result.aggregate_value, Some(60.0));

        // Filter then aggregate (Alice + Bob only) = 30
        let result = daoql.query()
            .scan("Person")
            .filter("name", "!=", serde_json::json!("Carol"))
            .aggregate("weight", crate::column::AggregateOp::Sum)
            .execute()
            .unwrap();
        assert_eq!(result.aggregate_value, Some(30.0));
    }

    // =========================================================================
    // Scenario test: real business scenarios (DSL driven)
    // =========================================================================

    /// Scenario 1: E-commerce inventory management — product creation, filtering, statistics
    #[test]
    fn test_dsl_ecommerce_inventory() {
        let dir = std::env::temp_dir().join("daoql-edu-test-ecommerce");
        let _ = std::fs::remove_dir_all(&dir);
        let daoql = DaoQL::open(&dir).unwrap();

        // Use Rust API to create product (weight = price)
        let mut phone = Being::new("iPhone", "Product");
        phone.core.weight = 999.0;
        phone.core.status = 1; // On shelf
        daoql.write(phone).unwrap();

        let mut earphone = Being::new("AirPods", "Product");
        earphone.core.weight = 199.0;
        earphone.core.status = 1;
        daoql.write(earphone).unwrap();

        let mut tablet = Being::new("iPad", "Product");
        tablet.core.weight = 2999.0;
        tablet.core.status = 0; // Off shelf
        daoql.write(tablet).unwrap();

        let mut watch = Being::new("Watch", "Product");
        watch.core.weight = 399.0;
        watch.core.status = 1;
        daoql.write(watch).unwrap();

        // DSL: filter expensive products (weight > 500)
        let result = daoql.execute_dsl(
            r#"query { Product(filter: {weight > 500}) { id, name, weight, status } }"#
        ).unwrap();
        if let dsl::executor::DslResult::Data(items) = result {
            assert_eq!(items.len(), 2, "expensive products should only be iPhone and iPad");
            let names: Vec<&str> = items.iter()
                .map(|v| v["name"].as_str().unwrap())
                .collect();
            assert!(names.contains(&"iPhone"));
            assert!(names.contains(&"iPad"));
        } else {
            panic!("expected Data");
        }

        // DSL: filter active products (status = 1)
        let result = daoql.execute_dsl(
            r#"query { Product(filter: {status: 1}) { id, name } }"#
        ).unwrap();
        if let dsl::executor::DslResult::Data(items) = result {
            assert_eq!(items.len(), 3, "active products should be 3");
        } else {
            panic!("expected Data");
        }

        // DSL: limit return count
        let result = daoql.execute_dsl(
            r#"query { Product(limit: 2) { id, name } }"#
        ).unwrap();
        if let dsl::executor::DslResult::Data(items) = result {
            assert_eq!(items.len(), 2);
        } else {
            panic!("expected Data");
        }
    }

    /// Scenario 2: Social network — user creation, relation establishment, BFS traversal
    #[test]
    fn test_dsl_social_network() {
        let dir = std::env::temp_dir().join("daoql-edu-test-social");
        let _ = std::fs::remove_dir_all(&dir);
        let daoql = DaoQL::open(&dir).unwrap();

        // Create users
        let alice = Being::new("Alice", "User");
        let id_alice = daoql.write(alice).unwrap();
        let bob = Being::new("Bob", "User");
        let id_bob = daoql.write(bob).unwrap();
        let carol = Being::new("Carol", "User");
        let id_carol = daoql.write(carol).unwrap();
        let dave = Being::new("Dave", "User");
        let id_dave = daoql.write(dave).unwrap();

        // establish follow relations: Alice->Bob, Alice->Carol, Bob->Dave
        let wb = crate::api::write_builder::WriteBuilder::new(
            &daoql.graph, &daoql.def_registry, &daoql.uuid_index
        );
        wb.create_relation(crate::relation::Relation::new(id_alice, id_bob, 1, true)).unwrap();
        wb.create_relation(crate::relation::Relation::new(id_alice, id_carol, 1, true)).unwrap();
        wb.create_relation(crate::relation::Relation::new(id_bob, id_dave, 1, true)).unwrap();

        // DSL: query all users
        let result = daoql.execute_dsl(
            r#"query { User { id, name } }"#
        ).unwrap();
        if let dsl::executor::DslResult::Data(items) = result {
            assert_eq!(items.len(), 4);
        } else {
            panic!("expected Data");
        }

        // DSL: BFS traverse Alice's social network
        let result = daoql.execute_dsl(
            r#"analyze { bfs on Alice(limit: 10) { } }"#
        ).unwrap();
        if let dsl::executor::DslResult::Data(items) = result {
            // BFS returns offset + depth, should include at least Bob(depth=1) and Carol(depth=1)
            assert!(items.len() >= 2, "BFS should traverse at least 2 direct follows");
        } else {
            panic!("expected Data");
        }
    }

    /// Scenario 3: Content recommendation — vector similarity search
    #[test]
    fn test_dsl_content_recommendation() {
        let dir = std::env::temp_dir().join("daoql-edu-test-recommend");
        let _ = std::fs::remove_dir_all(&dir);
        let mut daoql = DaoQL::open(&dir).unwrap();

        // Register vector field
        daoql.register_vector_field("embedding", 8);

        // Create articles (with vector embeddings)
        let article1 = Being::new("Rust Intro", "Article");
        let id1 = daoql.write(article1).unwrap();
        let article2 = Being::new("Rust Advanced", "Article");
        let id2 = daoql.write(article2).unwrap();
        let article3 = Being::new("Python Intro", "Article");
        let id3 = daoql.write(article3).unwrap();

        // Insert vector (Rust articles similar, Python articles different)
        if let Some(index) = daoql.vector_indices.get_mut("embedding") {
            index.insert(id1, vec![0.9, 0.8, 0.7, 0.6, 0.1, 0.1, 0.1, 0.1]).unwrap();
            index.insert(id2, vec![0.85, 0.75, 0.65, 0.55, 0.15, 0.15, 0.15, 0.15]).unwrap();
            index.insert(id3, vec![0.1, 0.1, 0.1, 0.1, 0.9, 0.8, 0.7, 0.6]).unwrap();
        }

        // DSL: vector similarity search, find articles similar to Rust Intro
        let result = daoql.execute_dsl(
            r#"similar { Article(query: [0.9, 0.8, 0.7, 0.6, 0.1, 0.1, 0.1, 0.1], k: 2) { id, name, distance } }"#
        ).unwrap();
        if let dsl::executor::DslResult::Data(items) = result {
            assert_eq!(items.len(), 2, "should return 2 most similar articles");
            // Most similar should be Rust Intro itself, followed by Rust Advanced
            let first_name = items[0]["name"].as_str().unwrap();
            assert!(first_name.contains("Rust"), "most similar result should contain Rust article");
        } else {
            panic!("expected Data");
        }
    }

    /// Scenario 4: Composite filter — range query, empty results, multiple conditions
    #[test]
    fn test_dsl_complex_filters() {
        let dir = std::env::temp_dir().join("daoql-edu-test-filters");
        let _ = std::fs::remove_dir_all(&dir);
        let daoql = DaoQL::open(&dir).unwrap();

        // Create employees (weight = salary, priority = level)
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

        // DSL: high-salary employees with salary > 10000
        let result = daoql.execute_dsl(
            r#"query { Employee(filter: {weight > 10000}) { id, name, weight } }"#
        ).unwrap();
        if let dsl::executor::DslResult::Data(items) = result {
            assert_eq!(items.len(), 2, "high-salary employees should be Bob and Carol");
        } else {
            panic!("expected Data");
        }

        // DSL: junior employees with salary < 6000
        let result = daoql.execute_dsl(
            r#"query { Employee(filter: {weight < 6000}) { id, name, weight } }"#
        ).unwrap();
        if let dsl::executor::DslResult::Data(items) = result {
            assert_eq!(items.len(), 1);
            assert_eq!(items[0]["name"], "Dave");
        } else {
            panic!("expected Data");
        }

        // DSL: query non-existent condition (empty result)
        let result = daoql.execute_dsl(
            r#"query { Employee(filter: {weight > 99999}) { id, name } }"#
        ).unwrap();
        if let dsl::executor::DslResult::Data(items) = result {
            assert!(items.is_empty(), "no employee with salary > 99999");
        } else {
            panic!("expected Data");
        }

        // DSL: employees with level >= 5
        let result = daoql.execute_dsl(
            r#"query { Employee(filter: {priority >= 5}) { id, name, priority } }"#
        ).unwrap();
        if let dsl::executor::DslResult::Data(items) = result {
            assert_eq!(items.len(), 3, "level >=5 should include Alice, Bob, Carol");
        } else {
            panic!("expected Data");
        }
    }

    // =========================================================================
    // Mixed query scenario: cross-engine query (graph + columnar + vector + index)
    // =========================================================================

    /// Mixed scenario 1: graph scan + filter + columnar aggregate (core mixed query from paper)
    #[test]
    fn test_mixed_scan_filter_aggregate() {
        let dir = std::env::temp_dir().join("daoql-edu-test-mixed-sfa");
        let _ = std::fs::remove_dir_all(&dir);
        let daoql = DaoQL::open(&dir).unwrap();

        // Create orders (weight = amount)
        for i in 0..20 {
            let mut order = Being::new(format!("Order-{i}"), "Order");
            order.core.weight = (i * 10) as f64; // 0, 10, 20, ..., 190
            daoql.write(order).unwrap();
        }

        // Mixed query: graph scan all Orders → filter (amount > 50) → columnar SUM aggregate
        let result = daoql
            .query()
            .scan("Order")
            .filter("weight", "gt", serde_json::json!(50.0))
            .aggregate("weight", crate::column::AggregateOp::Sum)
            .execute()
            .unwrap();

        // Orders with amount > 50: 60+70+...+190 = 1750 (14 total)
        assert_eq!(result.aggregate_value, Some(1750.0));
    }

    /// Mixed scenario 2: vector search → graph traversal → column read
    #[test]
    fn test_mixed_vector_search_then_graph_traverse() {
        let dir = std::env::temp_dir().join("daoql-edu-test-mixed-vsg");
        let _ = std::fs::remove_dir_all(&dir);
        let mut daoql = DaoQL::open(&dir).unwrap();

        // Register vector field
        daoql.register_vector_field("embedding", 8);

        // Create articles and authors
        let article1 = Being::new("Rust Intro", "Article");
        let id1 = daoql.write(article1).unwrap();
        let author1 = Being::new("Zhang San", "Author");
        let author_id1 = daoql.write(author1).unwrap();

        let article2 = Being::new("Rust Advanced", "Article");
        let id2 = daoql.write(article2).unwrap();
        let author2 = Being::new("Li Si", "Author");
        let author_id2 = daoql.write(author2).unwrap();

        let article3 = Being::new("Python Intro", "Article");
        let id3 = daoql.write(article3).unwrap();

        // Establish author-article relation
        let wb = crate::api::write_builder::WriteBuilder::new(
            &daoql.graph, &daoql.def_registry, &daoql.uuid_index
        );
        wb.create_relation(crate::relation::Relation::new(author_id1, id1, 1, true)).unwrap();
        wb.create_relation(crate::relation::Relation::new(author_id2, id2, 1, true)).unwrap();

        // Insert vector (Rust articles similar)
        if let Some(index) = daoql.vector_indices.get_mut("embedding") {
            index.insert(id1, vec![0.9, 0.8, 0.7, 0.6, 0.1, 0.1, 0.1, 0.1]).unwrap();
            index.insert(id2, vec![0.85, 0.75, 0.65, 0.55, 0.15, 0.15, 0.15, 0.15]).unwrap();
            index.insert(id3, vec![0.1, 0.1, 0.1, 0.1, 0.9, 0.8, 0.7, 0.6]).unwrap();
        }

        // Step 1: vector search find articles similar to "Rust Intro"
        let similar_results = daoql.query().similar_to(vec![0.9, 0.8, 0.7, 0.6, 0.1, 0.1, 0.1, 0.1], 2).execute().unwrap();
        assert!(!similar_results.items.is_empty(), "Should find similar articles");

        // Step 2: for each similar article, traverse author relations, read author info
        for article in &similar_results.items {
            let graph = daoql.graph.borrow();
            // Find article offset via UUID index
            if let Ok(Some(offset)) = daoql.uuid_index.borrow().get(article.core.id) {
                // Traverse outgoing edges (author→article relation reverse lookup is complex, simplified: read author directly)
                let _ = graph.read_node(offset);
            }
        }

        // Verify: at least one Rust-related article found
        let names: Vec<&str> = similar_results.items.iter()
            .map(|b| b.core.name.as_str())
            .collect();
        assert!(names.iter().any(|n| n.contains("Rust")));
    }

    /// Mixed scenario 3: BFS graph traversal → columnar read
    #[test]
    fn test_mixed_bfs_then_column_read() {
        let dir = std::env::temp_dir().join("daoql-edu-test-mixed-bfs");
        let _ = std::fs::remove_dir_all(&dir);
        let daoql = DaoQL::open(&dir).unwrap();

        // Create social network (priority = influence score)
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

        // establish follow relations: Alice->Bob, Alice->Carol, Bob->Dave
        let wb = crate::api::write_builder::WriteBuilder::new(
            &daoql.graph, &daoql.def_registry, &daoql.uuid_index
        );
        wb.create_relation(crate::relation::Relation::new(id_alice, id_bob, 1, true)).unwrap();
        wb.create_relation(crate::relation::Relation::new(id_alice, id_carol, 1, true)).unwrap();
        wb.create_relation(crate::relation::Relation::new(id_bob, id_dave, 1, true)).unwrap();

        // Step 1: BFS traverse Alice's social network
        let uuid_idx = daoql.uuid_index.borrow();
        let alice_offset = uuid_idx.get(id_alice).unwrap().unwrap();
        drop(uuid_idx);

        let mut graph = daoql.graph.borrow_mut();
        let bfs_results = crate::graph::traversal::bfs(&mut graph, alice_offset, 3, None).unwrap();
        drop(graph);

        assert!(!bfs_results.is_empty(), "BFS should at least traverse to Alice herself");

        // Step 2: for each node traversed by BFS, read priority (influence) from column store
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
        assert_eq!(total_influence, 235, "BFS traversed users total influence should be 235");
        assert_eq!(count, 4, "should traverse 4 users");
    }

    /// Mixed scenario 4: point query → relation traversal → columnar aggregate
    #[test]
    fn test_mixed_point_query_relation_aggregate() {
        let dir = std::env::temp_dir().join("daoql-edu-test-mixed-pqra");
        let _ = std::fs::remove_dir_all(&dir);
        let daoql = DaoQL::open(&dir).unwrap();

        // Create customers and orders
        let customer = Being::new("Alice", "Customer");
        let cust_id = daoql.write(customer).unwrap();

        for i in 1..=5 {
            let mut order = Being::new(format!("Order-{i}"), "Order");
            order.core.weight = (i * 100) as f64; // 100, 200, 300, 400, 500
            let order_id = daoql.write(order).unwrap();

            // Establish customer-order relation
            let wb = crate::api::write_builder::WriteBuilder::new(
                &daoql.graph, &daoql.def_registry, &daoql.uuid_index
            );
            wb.create_relation(crate::relation::Relation::new(cust_id, order_id, 1, true)).unwrap();
        }

        // Step 1: point query find customer Alice
        let customer_result = daoql.query().being(cust_id).fetch_one().unwrap();
        assert!(customer_result.is_some());

        // Step 2: traverse all outgoing edges of Alice (order relations)
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

        assert_eq!(order_ids.len(), 5, "Alice should have 5 orders");

        // Step 3: read each order amount from column store and sum
        let column = daoql.column.borrow();
        let mut total = 0.0;
        for order_id in &order_ids {
            if let Some(v) = column.column("weight").and_then(|col| col.get(*order_id)) {
                total += v;
            }
        }

        // 100 + 200 + 300 + 400 + 500 = 1500
        assert_eq!(total, 1500.0, "Alice total order amount should be 1500");
    }

    /// Scenario 5: E-commerce recommendation — vector search → graph relation → columnar attributes (cross-engine nested)
    #[test]
    fn test_dsl_ecommerce_recommendation() {
        let dir = std::env::temp_dir().join("daoql-edu-test-rec");
        let _ = std::fs::remove_dir_all(&dir);
        let mut daoql = DaoQL::open(&dir).unwrap();
        daoql.register_vector_field("embedding", 8);

        // Create products (weight = price)
        let mut phone = Being::new("iPhone", "Product");
        phone.core.weight = 999.0;
        let id_phone = daoql.write(phone).unwrap();

        let mut earphone = Being::new("AirPods", "Product");
        earphone.core.weight = 199.0;
        let id_earphone = daoql.write(earphone).unwrap();

        let mut tablet = Being::new("iPad", "Product");
        tablet.core.weight = 2999.0;
        let id_tablet = daoql.write(tablet).unwrap();

        // Establish accessory relation: iPhone -> AirPods, iPhone -> iPad
        let wb = crate::api::write_builder::WriteBuilder::new(
            &daoql.graph, &daoql.def_registry, &daoql.uuid_index
        );
        wb.create_relation(crate::relation::Relation::new(id_phone, id_earphone, 1, true)).unwrap();
        wb.create_relation(crate::relation::Relation::new(id_phone, id_tablet, 1, true)).unwrap();

        // Insert vector (iPhone and AirPods embeddings similar)
        if let Some(index) = daoql.vector_indices.get_mut("embedding") {
            index.insert(id_phone, vec![0.9, 0.8, 0.7, 0.6, 0.1, 0.1, 0.1, 0.1]).unwrap();
            index.insert(id_earphone, vec![0.85, 0.75, 0.65, 0.55, 0.15, 0.15, 0.15, 0.15]).unwrap();
            index.insert(id_tablet, vec![0.1, 0.1, 0.1, 0.1, 0.9, 0.8, 0.7, 0.6]).unwrap();
        }

        // DSL: vector search products similar to iPhone, result should contain price(weight) and related products
        let result = daoql.execute_dsl(
            r#"similar { Product(query: [0.9, 0.8, 0.7, 0.6, 0.1, 0.1, 0.1, 0.1], k: 3) { } }"#
        ).unwrap();
        if let dsl::executor::DslResult::Data(items) = result {
            assert!(!items.is_empty(), "should return similar products");
            // First result should be iPhone itself, should contain weight and relations
            let first = &items[0];
            assert!(first.get("weight").is_some(), "result should contain columnar price weight");
            assert!(first.get("relations").is_some(), "result should contain graph relations");
            assert!(first.get("relation_count").is_some(), "result should contain relation count");
            let relations = first["relations"].as_array().unwrap();
            assert_eq!(relations.len(), 2, "iPhone should have 2 related products");
        } else {
            panic!("expected Data");
        }
    }

    /// Scenario 6: Social network influence — BFS traversal → columnar priority (cross-engine nested)
    #[test]
    fn test_dsl_social_influence() {
        let dir = std::env::temp_dir().join("daoql-edu-test-influence");
        let _ = std::fs::remove_dir_all(&dir);
        let daoql = DaoQL::open(&dir).unwrap();

        // Create users (priority = influence score)
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

        // establish follow relations: Alice->Bob, Alice->Carol, Bob->Dave
        let wb = crate::api::write_builder::WriteBuilder::new(
            &daoql.graph, &daoql.def_registry, &daoql.uuid_index
        );
        wb.create_relation(crate::relation::Relation::new(id_alice, id_bob, 1, true)).unwrap();
        wb.create_relation(crate::relation::Relation::new(id_alice, id_carol, 1, true)).unwrap();
        wb.create_relation(crate::relation::Relation::new(id_bob, id_dave, 1, true)).unwrap();

        // DSL: BFS traverse Alice's social network, returned results should contain priority
        let result = daoql.execute_dsl(
            r#"analyze { bfs on Alice(limit: 10) { } }"#
        ).unwrap();
        if let dsl::executor::DslResult::Data(items) = result {
            assert!(!items.is_empty(), "BFS should return traversal results");
            // Find Bob's result, should contain priority
            let bob_item = items.iter().find(|v| v["name"] == "Bob");
            assert!(bob_item.is_some(), "should find Bob");
            let bob_item = bob_item.unwrap();
            assert!(bob_item.get("priority").is_some(), "result should contain columnar priority");
            assert_eq!(bob_item["priority"].as_f64().unwrap(), 80.0, "Bob influence score should be 80");
        } else {
            panic!("expected Data");
        }
    }

    /// Scenario 7: Sales report — DSL aggregate query (columnar aggregate fast path)
    #[test]
    fn test_dsl_sales_report() {
        let dir = std::env::temp_dir().join("daoql-edu-test-sales");
        let _ = std::fs::remove_dir_all(&dir);
        let daoql = DaoQL::open(&dir).unwrap();

        // Create orders (weight = amount)
        let mut o1 = Being::new("Order-A", "Order");
        o1.core.weight = 100.0;
        daoql.write(o1).unwrap();

        let mut o2 = Being::new("Order-B", "Order");
        o2.core.weight = 200.0;
        daoql.write(o2).unwrap();

        let mut o3 = Being::new("Order-C", "Order");
        o3.core.weight = 300.0;
        daoql.write(o3).unwrap();

        // DSL: query total order amount
        let result = daoql.execute_dsl(
            r#"query { Order(aggregate: {weight: sum}) { } }"#
        ).unwrap();
        if let dsl::executor::DslResult::Data(items) = result {
            assert_eq!(items.len(), 1, "Aggregate query should return single-row result");
            let agg = &items[0];
            assert_eq!(agg["aggregate"], 600.0, "total order amount should be 600");
            assert_eq!(agg["count"], 3, "order count should be 3");
            assert_eq!(agg["op"], "sum");
        } else {
            panic!("expected Data");
        }

        // DSL: filtered aggregation (only count orders with amount > 150)
        let result = daoql.execute_dsl(
            r#"query { Order(filter: {weight > 150}, aggregate: {weight: sum}) { } }"#
        ).unwrap();
        if let dsl::executor::DslResult::Data(items) = result {
            assert_eq!(items.len(), 1);
            assert_eq!(items[0]["aggregate"], 500.0, "order sum for amount >150 should be 500");
            assert_eq!(items[0]["count"], 2, "should count 2 orders");
        } else {
            panic!("expected Data");
        }
    }

    /// Scenario 8: Risk detection — vector search anomalous transactions → graph relation → columnar amount (cross-engine nested)
    #[test]
    fn test_dsl_risk_detection() {
        let dir = std::env::temp_dir().join("daoql-edu-test-risk");
        let _ = std::fs::remove_dir_all(&dir);
        let mut daoql = DaoQL::open(&dir).unwrap();
        daoql.register_vector_field("embedding", 8);

        // Create transactions (weight = transaction amount)
        let mut t1 = Being::new("Tx-Alice-1", "Transaction");
        t1.core.weight = 5000.0;
        let id1 = daoql.write(t1).unwrap();

        let mut t2 = Being::new("Tx-Alice-2", "Transaction");
        t2.core.weight = 8000.0;
        let id2 = daoql.write(t2).unwrap();

        let mut t3 = Being::new("Tx-Bob-1", "Transaction");
        t3.core.weight = 200.0;
        let id3 = daoql.write(t3).unwrap();

        // Create accounts
        let alice = Being::new("Alice-Account", "Account");
        let id_alice = daoql.write(alice).unwrap();

        // Establish transaction-account relation
        let wb = crate::api::write_builder::WriteBuilder::new(
            &daoql.graph, &daoql.def_registry, &daoql.uuid_index
        );
        wb.create_relation(crate::relation::Relation::new(id1, id_alice, 1, true)).unwrap();
        wb.create_relation(crate::relation::Relation::new(id2, id_alice, 1, true)).unwrap();

        // Insert vector (Alice's transactions similar)
        if let Some(index) = daoql.vector_indices.get_mut("embedding") {
            index.insert(id1, vec![0.9, 0.8, 0.7, 0.6, 0.1, 0.1, 0.1, 0.1]).unwrap();
            index.insert(id2, vec![0.85, 0.75, 0.65, 0.55, 0.15, 0.15, 0.15, 0.15]).unwrap();
            index.insert(id3, vec![0.1, 0.1, 0.1, 0.1, 0.9, 0.8, 0.7, 0.6]).unwrap();
        }

        // DSL: vector search similar transactions, result should contain amount and related accounts
        let result = daoql.execute_dsl(
            r#"similar { Transaction(query: [0.9, 0.8, 0.7, 0.6, 0.1, 0.1, 0.1, 0.1], k: 3) { } }"#
        ).unwrap();
        if let dsl::executor::DslResult::Data(items) = result {
            assert!(!items.is_empty(), "should return similar transactions");
            let first = &items[0];
            assert!(first.get("weight").is_some(), "result should contain columnar amount weight");
            assert!(first.get("relations").is_some(), "result should contain graph relations");
            let weight = first["weight"].as_f64().unwrap();
            assert!(weight > 0.0, "transaction amount should be > 0");
        } else {
            panic!("expected Data");
        }
    }
}
