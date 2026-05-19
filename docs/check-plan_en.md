<!--
Copyright 2026 Zhanbo Li / Atlas Lee <zhanbo.lee@gmail.com>

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



# DaoQL-Edu Inspection Plan

> **Version**: v1.0  
> **Date**: 2026-05-18  
> **Status**: Confirmed  
> **License**: BSL 1.1

---

## 1. Overview

This document defines the inspection criteria, test strategy, and quality gates for the DaoQL-Edu project during **Phase 4 (Implementation)** and **Phase 5 (Verification)**.

**Inspection Objectives**:
- Ensure code conforms to the architecture design document (docs/ARCHITECTURE.md)
- Ensure code conforms to Rust coding standards
- Ensure functional coverage > 80%
- Ensure critical algorithms have correctness verification
- Ensure performance benchmarks are met

---

## 2. Coding Standards Checklist

### 2.1 Basic Standards (Mandatory)

| # | Check Item | Standard | Verification Method |
|---|------------|----------|---------------------|
| FMT-01 | Code formatting | `cargo fmt` produces no changes | `cargo fmt -- --check` |
| FMT-02 | Line width | 120 characters | `rustfmt.toml: max_width = 120` |
| FMT-03 | Indentation | 4 spaces | `rustfmt.toml: tab_spaces = 4` |
| FMT-04 | Trailing commas | Trailing commas for multi-line structs/enums | `rustfmt.toml: trailing_comma = "Vertical"` |

### 2.2 Clippy Rules (Mandatory)

```bash
# Check command
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

| # | Check Item | Severity | Description |
|---|------------|----------|-------------|
| CLIP-01 | `unwrap()` / `expect()` usage | ⚠️ Warning | Raw unwrap is prohibited in production code; allowed in test code |
| CLIP-02 | `panic!()` in functions returning Result | ⚠️ Warning | Should return Err instead of panicking |
| CLIP-03 | `todo!()` / `unimplemented!()` | ⚠️ Warning | Must be removed or tagged as an ISSUE before committing |
| CLIP-04 | Unused variables/imports | ❌ Deny | `-D warnings` |
| CLIP-05 | Optimizable code patterns | ℹ️ Pedantic | E.g., `filter_map` instead of `filter().map()`, etc. |

### 2.3 Documentation Standards (Mandatory)

| # | Check Item | Standard | Verification Method |
|---|------------|----------|---------------------|
| DOC-01 | Public API documentation | All `pub` items must have rustdoc | `cargo doc` produces no missing-docs warning |
| DOC-02 | Module-level educational comments | Each module header ≥ 3 lines of comments explaining core concepts | Manual inspection |
| DOC-03 | Algorithm comments | Critical algorithms (HNSW/ClockSweep/SIMD/GroupCommit) have step-by-step derivations | Manual inspection |
| DOC-04 | unsafe block comments | Each `unsafe` block must have a `// SAFETY:` explanation | `grep -r "unsafe" src/ + manual inspection` |

### 2.4 Code Structure Standards (Mandatory)

| # | Check Item | Standard |
|---|------------|----------|
| STRUCT-01 | Lines per file | ≤ 1000 lines (critical modules may be relaxed to 1200) |
| STRUCT-02 | Lines per function | ≤ 80 lines (complex algorithms may be relaxed to 120, with explanatory comments) |
| STRUCT-03 | unsafe code isolation | unsafe is allowed only in `storage/mmap.rs` and `vector/distance.rs` |
| STRUCT-04 | Module dependency direction | Lower-level modules must not depend on upper-level ones (refer to ARCHITECTURE.md dependency graph) |
| STRUCT-05 | Unified error handling | All errors must use the `DaoQLError` enum; raw `Box<dyn Error>` is prohibited |

---

## 3. Test Strategy

### 3.1 Test Pyramid

```
         ┌─────────┐
         │ E2E/Integration │  ← 15% coverage target
         │   (Slow)        │     tests/integration_*.rs
         ├─────────┤
         │ Integration Tests │  ← 25% coverage target
         │   (Medium)        │     tests/*_tests.rs
         ├─────────┤
         │ Unit Tests        │  ← 60% coverage target
         │   (Fast)          │     #[cfg(test)] inside src/*/mod.rs
         └─────────┘
```

### 3.2 Unit Tests (`#[cfg(test)]`)

**Required unit tests for each module**:

| Module | Required Tests | Min Test Cases |
|--------|---------------|----------------|
| `id.rs` | BeingId generation, serialization, comparison | 5 |
| `being.rs` | BeingCore ↔ NodeRecord conversion | 5 |
| `def.rs` | Field type validation, constraint checks | 5 |
| `version.rs` | tx_begin/tx_end visibility determination | 5 |
| `storage/mmap.rs` | Allocation, read/write, expansion | 5 |
| `graph/record.rs` | NodeRecord/EdgeRecord size assertions | 2 |
| `graph/store.rs` | Create/read nodes, edges | 5 |
| `graph/traversal.rs` | BFS, DFS correctness | 5 |
| `column/raw_layer.rs` | Append, read, compress/decompress | 5 |
| `column/projected_layer.rs` | Projected column registration, scan | 5 |
| `column/skip_index.rs` | min/max pruning correctness | 5 |
| `column/aggregation.rs` | SUM/AVG/COUNT correctness, SIMD vs scalar consistency | 10 |
| `vector/hnsw.rs` | Insert, search, recall rate | 10 |
| `vector/distance.rs` | Cosine/L2/Dot correctness, SIMD vs scalar consistency | 10 |
| `vector/quantization.rs` | SQ/BQ precision loss is controllable | 5 |
| `index/uuid_index.rs` | Insert, query, delete | 5 |
| `wal/writer.rs` | Append, double-buffer switch, CRC check | 5 |
| `wal/recovery.rs` | Crash recovery, idempotent replay | 5 |
| `pagecache/clock_sweep.rs` | Hit, eviction, concurrency safety | 5 |
| `transaction/lock_manager.rs` | Lock ordering, deadlock avoidance | 5 |
| `transaction/committer.rs` | Two-phase commit flow | 5 |
| `dsl/lexer.rs` | Lexical analysis correctness | 10 |
| `dsl/parser.rs` | Syntactic analysis correctness, error recovery | 10 |
| `dsl/executor.rs` | AST → query plan → execution | 5 |
| `api/query_builder.rs` | Chained API composition | 5 |

### 3.3 Integration Tests (`tests/`)

| Test File | Test Scenario | Min Test Cases |
|-----------|--------------|----------------|
| `integration_tests.rs` | Full lifecycle: Def → Being → Relation → Query → Version | 5 |
| `graph_tests.rs` | Graph traversal depth, PageRank convergence | 5 |
| `column_tests.rs` | Column scan filtering, aggregation GROUP BY, Skip Index pruning effectiveness | 5 |
| `vector_tests.rs` | HNSW recall rate > 90% (100k random vectors Top10) | 3 |
| `transaction_tests.rs` | Concurrent write conflicts, WAL recovery, ACID verification | 5 |
| `dsl_tests.rs` | End-to-end execution of all DSL syntax | 10 |

### 3.4 Stress Tests (`benches/`)

| Benchmark | Objective | Scale |
|-----------|-----------|-------|
| `bench_insert.rs` | Bulk write throughput | 1 million Beings |
| `bench_graph_traverse.rs` | Graph traversal latency | 100k nodes, average degree 5 |
| `bench_column_aggregate.rs` | Column aggregation latency | 1 million rows, GROUP BY 10 groups |
| `bench_vector_search.rs` | Vector search latency | 100k vectors, dim=384, TopK=10 |
| `bench_point_query.rs` | Point query latency | 1 million Beings, random queries |

**Performance Targets** (refer to requirements document §9.1):

| Scenario | Target | Acceptable Range |
|----------|--------|------------------|
| Point query | < 1ms | < 5ms |
| Graph traversal 30 hops | < 1ms | < 5ms |
| Vector search TopK 10 | < 10ms | < 50ms |
| Column aggregation 100k rows | < 100ms | < 500ms |
| Bulk write 10k/sec | — | > 1000/sec |

---

## 4. Quality Gates

### 4.1 Pre-commit Checks

```bash
#!/bin/bash
# .githooks/pre-commit (or run manually)

set -e

echo "🔍 Checking code formatting..."
cargo fmt -- --check

echo "🔍 Running Clippy..."
cargo clippy --all-targets --all-features -- -D warnings \
  -W clippy::unwrap_used \
  -W clippy::expect_used \
  -W clippy::panic_in_result_fn \
  -W clippy::todo

echo "🔍 Running unit tests..."
cargo test --lib

echo "🔍 Building documentation..."
cargo doc --no-deps

echo "✅ Pre-commit checks passed"
```

### 4.2 Pre-merge Checks

| # | Check Item | Pass Criteria |
|---|------------|---------------|
| PM-01 | Unit tests | All `cargo test --lib` pass |
| PM-02 | Integration tests | All `cargo test --test '*'` pass |
| PM-03 | Doc tests | All `cargo test --doc` pass |
| PM-04 | Clippy | Zero warnings (including pedantic) |
| PM-05 | Formatting | `cargo fmt -- --check` passes |
| PM-06 | Coverage | `cargo tarpaulin --lib` ≥ 80% |
| PM-07 | Benchmarks | Stress tests run without panic, performance within acceptable range |

### 4.3 Coverage Tools

```bash
# Using cargo-tarpaulin (Linux)
cargo install cargo-tarpaulin
cargo tarpaulin --lib --out Html --out Stdout

# Or cargo-llvm-cov (cross-platform)
cargo install cargo-llvm-cov
cargo llvm-cov --lib --html
```

**Coverage Targets**:
- Overall line coverage ≥ 80%
- Core engine (graph/column/vector) line coverage ≥ 85%
- unsafe code path coverage = 100%

---

## 5. Design Conformance Checks

### 5.1 Architecture Conformance

| # | Check Item | Verification Method |
|---|------------|---------------------|
| ARCH-01 | Module directory matches ARCHITECTURE.md §3.1 | `tree src/ && diff docs/ARCHITECTURE.md` |
| ARCH-02 | Module dependency direction is correct (no reverse dependencies) | `cargo modules generate tree --lib` |
| ARCH-03 | Core data structure sizes are correct | `assert_eq!(size_of::<NodeRecord>(), 1536)` compile-time assertion |
| ARCH-04 | File layout matches ARCHITECTURE.md §11 | `tree .` |

### 5.2 Requirements Traceability Matrix

| Requirement ID | Requirement Description | Design Doc | Implementation Module | Test File | Status |
|----------------|------------------------|------------|----------------------|-----------|--------|
| R-3.1 | Being primitive | §4.1 | `being.rs` | `being_tests.rs` | ⬜ |
| R-3.2 | Def primitive | §4.4 | `def.rs` | `def_tests.rs` | ⬜ |
| R-3.3 | Relation primitive | §4.5 | `relation.rs` | `relation_tests.rs` | ⬜ |
| R-3.4 | Version MVCC | §4.2 | `version.rs` | `version_tests.rs` | ⬜ |
| R-4.1 | Graph processing | §5 / §9.1 | `graph/` | `graph_tests.rs` | ⬜ |
| R-4.2 | Column processing | §5 / §9.3 | `column/` | `column_tests.rs` | ⬜ |
| R-4.3 | Document processing | §5 | `column/raw_layer.rs` | `column_tests.rs` | ⬜ |
| R-4.4 | Vector processing | §5 / §9.1 | `vector/` | `vector_tests.rs` | ⬜ |
| R-5.1 | ACID transactions | §7 | `transaction/` | `transaction_tests.rs` | ⬜ |
| R-5.2 | WAL | §7.2 / §9.4 | `wal/` | `wal_tests.rs` | ⬜ |
| R-6.1 | Fluent API | §10.2 | `api/` | `api_tests.rs` | ⬜ |
| R-6.2 | DSL | §8 / `dsl/` | `dsl/` | `dsl_tests.rs` | ⬜ |
| R-7.1 | UUID→Offset index | §6.1 | `index/uuid_index.rs` | `index_tests.rs` | ⬜ |
| R-7.2 | Time Range index | §6.2 | `index/time_index.rs` | `index_tests.rs` | ⬜ |
| R-7.3 | Skip Index | §6.3 | `column/skip_index.rs` | `skip_index_tests.rs` | ⬜ |
| R-8.1 | PageCache | §9.2 | `pagecache/` | `pagecache_tests.rs` | ⬜ |
| R-9.1 | Performance benchmarks | §3.4 | `benches/` | `benchmark.rs` | ⬜ |
| R-11.1 | Acceptance criteria | — | All | `integration_tests.rs` | ⬜ |

---

## 6. Safety and Stability Checks

### 6.1 unsafe Code Audit

| # | Check Item | Standard |
|---|------------|----------|
| UNSAFE-01 | unsafe code is centralized | Only in `storage/mmap.rs` and `vector/distance.rs` |
| UNSAFE-02 | Bounds checks | Length/range checks precede every unsafe pointer operation |
| UNSAFE-03 | SAFETY comments | Each `unsafe` block is preceded by a `// SAFETY:` explanation |
| UNSAFE-04 | Miri check | `cargo miri test` passes memory safety checks |

```bash
# Miri check command
rustup component add miri
cargo miri test --lib
```

### 6.2 Concurrency Safety

| # | Check Item | Verification Method |
|---|------------|---------------------|
| CONC-01 | `Send + Sync` correctness | Compiler checks (data engine implements `Send + Sync`) |
| CONC-02 | Lock granularity is reasonable | Code review: PerBeingLock instead of global locks |
| CONC-03 | Deadlock avoidance | Test: transactions acquire locks in BeingId order |
| CONC-04 | Data races | `cargo miri test` + `loom` (optional) |

### 6.3 Resource Leaks

| # | Check Item | Verification Method |
|---|------------|---------------------|
| LEAK-01 | File descriptors | Check `/proc/self/fd` after `cargo test` (Linux) |
| LEAK-02 | mmap release | munmap on process exit (Drop implementation) |
| LEAK-03 | Lock release | `Drop` implementation ensures locks are released even on panic |

---

## 7. Delivery Checklist

### 7.1 Code Delivery

| # | Check Item | Status |
|---|------------|--------|
| DLV-01 | `cargo build` succeeds | ⬜ |
| DLV-02 | All `cargo test` pass | ⬜ |
| DLV-03 | `cargo clippy` zero warnings | ⬜ |
| DLV-04 | `cargo fmt --check` passes | ⬜ |
| DLV-05 | `cargo doc` no missing-docs | ⬜ |
| DLV-06 | Coverage ≥ 80% | ⬜ |
| DLV-07 | Miri check passes | ⬜ |

### 7.2 Documentation Delivery

| # | Check Item | Status |
|---|------------|--------|
| DLV-08 | README.md (quick start) | ⬜ |
| DLV-09 | docs/ARCHITECTURE.md (architecture design) | ✅ |
| DLV-10 | docs/API.md (API manual) | ⬜ |
| DLV-11 | docs/requirements.md (requirements document) | ✅ |
| DLV-12 | Per-module educational comments | ⬜ |

### 7.3 Feature Delivery

| # | Check Item | Status |
|---|------------|--------|
| DLV-13 | Def definition and constraints | ⬜ |
| DLV-14 | Being creation (core + ext + embedding) | ⬜ |
| DLV-15 | Relation creation (directed/undirected) | ⬜ |
| DLV-16 | Graph traversal (BFS/DFS) | ⬜ |
| DLV-17 | Column aggregation (SUM/AVG/COUNT + GROUP BY) | ⬜ |
| DLV-18 | Vector similarity search (HNSW TopK) | ⬜ |
| DLV-19 | Version history (history / as_of) | ⬜ |
| DLV-20 | DSL complete pipeline | ⬜ |

---

## 8. Inspection Execution Plan

### 8.1 Inspection Rhythm During Phase 4 (Implementation)

```
After each module is implemented:
  ├─ cargo fmt
  ├─ cargo clippy
  ├─ cargo test --lib (unit tests for that module)
  └─ cargo doc

After each engine is implemented (graph/column/vector/index/wal):
  ├─ All checks above
  ├─ cargo test --test (corresponding integration tests)
  ├─ cargo tarpaulin (coverage snapshot)
  └─ Update requirements traceability matrix

After all implementations are complete:
  ├─ Full test suite
  ├─ Stress tests
  ├─ Miri check
  ├─ Coverage report
  └─ Design conformance review
```

### 8.2 Inspection Toolchain

```toml
# Cargo.toml [dev-dependencies]
[dev-dependencies]
criterion = "0.5"          # Benchmarking
cargo-tarpaulin = "0.27"   # Coverage (Linux dev machine)
proptest = "1.4"           # Property testing
rand = "0.8"               # Test data generation
```

---

*This document serves as the inspection baseline for Phases 4–5; items are verified step by step during execution.*
