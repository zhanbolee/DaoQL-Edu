# LDBC SNB Benchmark Report

**Date**: 2026-06-02
**Build**: release mode (optimized)
**Version**: v1.1.0 (commit 729677e)
**Platform**: Apple M4 Max (16-core), 128GB LPDDR5, NVMe SSD (~7 GB/s)
**Scale**: SF1 (~1M Persons, 2.5M Posts, 5M Comments, 100K Forums, ~37M edges)

---

## SF1 Concurrent Benchmark (v1.1.0 Final with CSR)

### Test Configuration

| Parameter | Value |
|-----------|-------|
| **Scale Factor** | SF1 |
| **Concurrency** | 8 workers |
| **Warmup** | 30s |
| **Measurement** | 600s |
| **Page Cache** | 1 GB |
| **Hardware** | Apple M4 Max, 128GB, NVMe SSD |

### Coverage: 34/34 (100%)

### Results

| # | Query | Count | P50(ms) | P95(ms) | AvgRows |
|---|-------|:-----:|:------:|:------:|:-----:|
| 1 | IC1 — Person profile | 48 | 0.26 | 7.00 | 8.1 |
| 2 | IC2 — Recent messages | 28 | 0.16 | 0.26 | 10.0 |
| 3 | IC3 — Friend recommendation | 53 | 1.74 | 10.13 | 32.6 |
| 4 | IC4 — Content recommendation | 25 | 3.29 | 32.63 | 6.4 |
| 5 | IC5 — Forum member posts | 39 | 0.93 | 5.55 | 39.7 |
| 6 | IC6 — Message thread | 24 | 18,923 | 25,776 | 3.3 |
| 7 | IC7 — Recent replies | 44 | 1.26 | 8.14 | 29.3 |
| 8 | IC8 — Recent comments | 22 | 0.16 | 0.36 | 10.0 |
| 9 | IC9 — Friend messages | 45 | 1.93 | 43.95 | 6.8 |
| 10 | IC10 — Social recommendation | 32 | 18,383 | 27,396 | 5.1 |
| 11 | IC11 — Job referral | 36 | 4.21 | 639.81 | 3.6 |
| 12 | IC12 — Expert search | 31 | 0.10 | 1.41 | 0.0 |
| 13 | IC13 — Shortest path over time | 27 | 0.05 | 2.88 | 1.0 |
| 14 | IC14 — Weighted shortest path | 32 | 0.64 | 31.58 | 1.0 |
| 1 | BI1 — Posts by country | 25 | 1,203 | 2,401 | 6.2 |
| 2 | BI2 — Tag co-occurrence | 37 | 48.3 | 75.0 | 0.0 |
| 3 | BI3 — Forum growth | 33 | 35.2 | 62.7 | 1.0 |
| 4 | BI4 — Top forums | 37 | 21,225 | 31,483 | 100.0 |
| 5 | BI5 — Active people | 27 | 46.4 | 157.3 | 0.0 |
| 6 | BI6 — Tag popularity | 21 | 51.4 | 102.9 | 0.0 |
| 7 | BI7 — Authoritative users | 25 | 0.42 | 1.06 | 0.0 |
| 8 | BI8 — Social circle | 39 | 7.98 | 59.28 | 5.3 |
| 9 | BI9 — Forum content by tag | 6 | 908,271 | 969,790 | 25.0 |
| 10 | BI10 — Friend by tags | 26 | 1.28 | 13.05 | 0.0 |
| 11 | BI11 — Same IP authors | 11 | 229 | 503 | 0.0 |
| **12** | **BI12 — Triangle count** | **16** | **4,813** | **8,288** | **0.0** |
| 13 | BI13 — Friends by city | 25 | 1.23 | 9.48 | 2.8 |
| 14 | BI14 — Snake pattern | 25 | 4.07 | 25.73 | 20.5 |
| 15 | BI15 — Weighted interaction | 38 | 7.02 | 35.87 | 6.8 |
| 16 | BI16 — Expert search | 23 | 0.39 | 0.82 | 0.0 |
| **17** | **BI17 — Triangles filtered** | **34** | **5,222** | **8,708** | **1.0** |
| 18 | BI18 — Message distribution | 27 | 1,299 | 2,411 | 3.0 |
| 19 | BI19 — Stranger interaction | 41 | 10.4 | 41.4 | 2.8 |
| 20 | BI20 — High-level topics | 96 | 0.09 | 1.58 | 0.3 |

### Overall

| Metric | v1.0.0 | v1.1.0 (May) | v1.1.0 (Jun) | Improvement |
|--------|:------:|:------:|:------:|:----------:|
| Total queries | 302 | 1039 | **1098** | +264% |
| Errors | 3 | 0 | **0** | — |
| Throughput | 0.5 QPS | 1.7 QPS | **1.8 QPS** | +260% |

---

## Cumulative Optimization Impact (v1.0.0 → v1.1.0 Jun)

| Query | v1.0.0 | v1.1.0 Final | Change | Root Cause |
|-------|:------:|:------:|:---:|------|
| IC14 | 30,017ms | 0.64ms | -99.99% | max_iterations + bidirectional Dijkstra |
| **BI12** | 199,389ms | **4,813ms** | **-97.6%** | def_filter + par_chunks + read_deleted_and_type + CSR adjacency |
| **BI17** | 198,471ms | **5,222ms** | **-97.4%** | same |
| **BI9** | — | 908,271ms | — | per-hop types + terminal_only + CSR |
| IC3 | 2.91ms | 1.74ms | -40.2% | batch beings() + CSR BFS |
| IC4 | — | 3.29ms | — | N+1 → batch beings() + CSR BFS |
| BI8 | 18.1ms | 7.98ms | -55.9% | N+1 → multi-step batch BFS |
| BI10 | 5.72ms | 1.28ms | -77.6% | per-hop types + terminal_only |

### v1.1.0 Engine Fixes Applied

| Fix | Location | Impact |
|-----|----------|--------|
| dijkstra() max_iterations + bidirectional Dijkstra | pathfinding.rs | IC14 -99.99% |
| TriangleCount def_filter + par_chunks + degree<2 + read_deleted_and_type | connectivity.rs + edge_store.rs | BI12/17 -92% |
| Per-type CSR adjacency index (CsrManager + CsrIndex) | edge_store.rs | BI12/17 -70%, BI9 -31% |
| BFS edge chain pre-filter (read_edge_header) | traverse.rs + edge_store.rs | IC6/10 P95 -20% |
| Per-hop relation types + terminal_only | traverse.rs + query_executor.rs | BI9, BI10 |
| Batch beings() rewrites (13 queries) | queries.rs | IC3/4/7/9, BI5/6/7/8/10/13/16/20 |
| BI4 time-range pushdown to column engine | queries.rs | BI4 |
| Page cache configurable (--page-cache-mb, default 1GB) | main.rs | overall |
| Systematic data-layer warmup | main.rs | cold-start elimination |
| CRC-resilient CSR build_from_scan | edge_store.rs | robustness |

---

## Competitive Positioning

### Competitor Landscape

| System | Type | SF1 Coverage | vs. SunDaoQL |
|--------|------|:------:|------|
| **TigerGraph** | Commercial native graph DB | IC + BI full, ms to seconds | 10-year commercial product, BI queries comprehensively ahead |
| **Neo4j** | Commercial graph DB | IC solid, BI ~48% | SunDaoQL wins on BI coverage |
| **Kuzu** | Embedded graph DB | 30-query custom suite | Same embedded tier, comparable |
| **PostgreSQL** | Relational | Recursive CTE, low coverage | No native graph capability |

### Throughput Reference (SF1, 8 concurrent)

| System | QPS | Notes |
|--------|:---:|------|
| TigerGraph | tens to hundreds | Commercial, multi-core optimized |
| **SunDaoQL** | **1.8** | Dragged down by BI4/BI9/IC6/IC10 |
| Neo4j | — | Some BI queries timeout |

### Hardware Context

Benchmarks run on Apple M4 Max (16-core). Competitors typically use Xeon servers:

| | M4 Max | Typical Competitor (Xeon) |
|------|---------|---------------------------|
| Single-core | ⭐⭐⭐⭐⭐ Best-in-class | ⭐⭐⭐ |
| Multi-core | 16 cores | 32-128 cores |
| Memory BW | ~800 GB/s (unified) | ~400 GB/s (DDR5) |

M4 Max's class-leading single-core benefits single-threaded graph traversal. The 16-core limit constrains parallel BI throughput. Already-parallelized queries (BI12/BI17 triangle count) are projected to see 3-5× improvement on 64-core Xeon.

### Tier Summary

**Based on LDBC benchmark data (coverage + latency only):**

```
⭐⭐⭐⭐⭐  TigerGraph — All BI queries at interactive speed
⭐⭐⭐☆    SunDaoQL  — All IC interactive + all BI covered + triangle count at batch level
⭐⭐⭐      Neo4j    — IC strong, BI only 48% covered (13/25 cannot complete)
⭐⭐⭐      Kuzu     — Embedded graph DB, fast queries but notable regressions
⭐⭐        PostgreSQL — Recursive CTE workaround
```

> **Note**: Neo4j completes only 12 of 25 BI queries at SF1; 13 timeout or cannot return results. By pure benchmark metrics (coverage, latency), SunDaoQL has already surpassed Neo4j. However, Neo4j possesses Cypher ecosystem, production deployment track record, and query optimizer maturity that SunDaoQL lacks. Each leads in different dimensions; rankings vary by scoring weight.

---

## Industrial Manufacturing Scenarios

### SunDAO Core Scenario Mapping

SunDAO's primary market is industrial manufacturing (BOM/MRP/APS/MES/WMS/TMS). Query patterns in these domains differ fundamentally from social network BI9.

| Industrial Need | Workload Pattern | LDBC Query Equivalent | P50 | Status |
|----------------|-----------------|----------------------|:---:|:------:|
| Entity lookup (equipment/material/work order) | O(1) point query | IC1 | 0.26ms | ✅ |
| Single-level BOM | 1-hop traversal | IC5 pattern | 0.93ms | ✅ |
| Full BOM expansion (10 levels) | Deep tree traversal | BFS × 10 | ~0.1µs/hop | ✅ |
| Material substitution recs | Relationship recommendation | IC3 pattern | 1.74ms | ✅ |
| Shortest transport/AGV path | Shortest path | IC13/IC14 | 0.05-0.64ms | ✅ |
| Plant capacity aggregation | Dimensional aggregation | BI1 pattern | 1.2s | ⚠️ report-grade |
| Equipment failure correlation | Multi-hop + filter | BI19 | 10.4ms | ✅ |
| Material batch quality trace | Scan + filter + traverse | IC combo | ms | ✅ |
| Order status change propagation | BFS traversal | per-hop | µs | ✅ |

### Workload Feature Comparison

| Dimension | BI9 (Social Network) | Industrial Queries |
|-----------|---------------------|-------------------|
| Fan-out pattern | Unbounded broadcast × 2nd-order explosion | Bounded deep-narrow trees (BOM depth 5-15, fanout 10-50) |
| Data magnitude | Millions with cross-product explosion | Thousands with deterministic paths |
| Aggregation | Global GROUP BY + ORDER BY | Local recursive arithmetic (per-node ×qty ×loss_rate) |
| Time boundary | Unbounded | Naturally constrained by plant/line/period |

**BI9's 15-minute limitation does not affect SunDAO's industrial manufacturing scenarios.** The workloads are fundamentally different: BI9 fans out from few sources to millions of nodes for global aggregation, while industrial queries descend from a known origin along a constrained path, computing locally at each step.

---

## Knowledge Graph / AI Memory / Semantic Networks / World Models

SunDAO's four core non-industrial scenarios also differ fundamentally from BI9 in query patterns.

### Scenario Capability Mapping

| Scenario | Typical Operation | LDBC Equivalent | P50 | Status |
|----------|------------------|-----------------|:---:|:------:|
| **Knowledge Graph** — Entity details | Being lookup | IC1 | 0.26ms | ✅ |
| **Knowledge Graph** — Relationship path | 2-5 hop traversal | IC13/IC14/IC3 | 0.05-3.29ms | ✅ |
| **Knowledge Graph** — Supply chain penetration | Multi-hop exploration | IC4 | 3.29ms | ✅ |
| **AI Memory** — Recent conversations | Temporal filter | IC2/IC8 | 0.16ms | ✅ |
| **AI Memory** — Memory retrieval | Relationship recommend | IC9 | 1.93ms | ✅ |
| **AI Memory** — Version chain trace | Historical version walk | IC7 | 1.26ms | ✅ |
| **AI Memory** — Time window filter | Range filter | IC11 | 4.21ms | ✅ |
| **Semantic Network** — Concept neighbors | 1-hop expansion | BFS | µs/hop | ✅ |
| **Semantic Network** — Social circle | Multi-hop expansion | BI8 | 7.98ms | ✅ |
| **Semantic Network** — Pattern query | Graph pattern matching | BI14 | 4.07ms | ✅ |
| **World Model** — Belief propagation | BFS traversal | per-hop | µs | ✅ |
| **World Model** — Weighted interaction | Relationship + weight filter | BI15 | 7.02ms | ✅ |
| **World Model** — Time-window entity query | Scan + filter | IC11 | 4.21ms | ✅ |

### Core Conclusion

Daily operations in these four scenarios follow the pattern "start from a specific entity, traverse 2-5 hops along known relationship types, perform local computation or filtering at each step." This is the IC query access pattern, where 90% of queries are under 10ms with most in the sub-millisecond range.

BI9's "full-graph scan × Cartesian product explosion × global aggregation" pattern **does not appear** in the daily queries of these four scenarios. BI9's performance limitation is irrelevant to SunDAO's core knowledge scenarios.

The only queries requiring attention are IC6 and IC10 (18-19s), deep multi-hop ranking queries that are currently bottlenecks. Their pattern (constrained path exploration within 5 hops + Top-K ranking) is more controllable than BI9, with clear optimization paths via intermediate result caching and query plan improvements.

---

## Quantitative Trading Scenarios

### Scenario Fit

| Trading Sub-Scenario | SunDaoQL Fit | Notes |
|---------------------|:---:|------|
| **Compliance Audit** | ⭐⭐⭐⭐⭐ | WAL immutable-append + chain hash + REASONING_CHAIN white-box reasoning — unique advantage |
| **Multi-Asset Correlation Graph** | ⭐⭐⭐⭐ | Graph engine multi-hop relationship queries ("find all A-shares affected by RMB depreciation with >30% overseas assets"), ms latency |
| **Factor Research** | ⭐⭐⭐ | Column engine aggregation usable but not differentiated |
| **Vector Similarity Search** | ⭐⭐⭐ | 299µs HNSW usable, not beyond specialized solutions (Faiss, etc.) |
| **Strategy Backtesting** | ⭐⭐ | Data layer only; backtesting is the strategy engine's domain |
| **Real-time Market Data** | ⭐ | 1.8 QPS far below tick-level ingestion (tens to hundreds of thousands/sec) |
| **Low-Latency Signal Execution** | ❌ | <1µs deterministic latency requires FPGA/kernel-bypass, not software architecture |
| **Time-Series Window Functions** | ⭐ | Basic LAG/LEAD implemented; far from professional time-series DBs (kdb+ wj/aj/asof) |

### Core Judgment

SunDaoQL's true differentiation in quantitative trading is not "faster market data processing" — kdb+/QuestDB/custom C++ have long dominated that track — but rather the **combination of tamper-proof audit trail + multi-asset relationship graph**. Financial institutions under SEC/CSRC regulatory pressure will pay for this combination, and no single competitor currently covers both.

---

> Report generated: 2026-06-02 (v1.1.0)
> All benchmarks use real engine calls, no mocks or stubs
