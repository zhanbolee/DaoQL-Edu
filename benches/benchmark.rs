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
//! DaoQL-Edu 压力测试
//!
//! 覆盖核心路径的真实性能测量：
//! - 写入：Being 单条/批量写入、关系链式建立
//! - 查询：点查、类型扫描、条件过滤、DSL 执行
//! - 图遍历：BFS（星型图）、DFS（链式图）
//! - 向量：HNSW 批量插入、近似最近邻搜索
//!
//! 设计原则：
//! - 所有数据在 setup 阶段通过真实 API 写入
//! - benchmark 只测量目标操作（查询/遍历/搜索）
//! - 写入类测试在独立临时目录中执行，避免交叉污染

use criterion::{
    black_box, criterion_group, criterion_main, BenchmarkId, Criterion, SamplingMode,
    Throughput,
};
use daoql_edu::graph::{bfs, dfs, EdgeFilter};
use daoql_edu::id::BeingId;
use daoql_edu::vector::hnsw::HnswIndex;
use daoql_edu::{Being, DaoQL};
use rand::seq::SliceRandom;
use rand::Rng;
use rand::SeedableRng;

// ===========================================================================
// 常量
// ===========================================================================

/// 向量维度（教学版使用 128 维以加速测试）
const DIM: usize = 128;
/// 随机种子，保证测试可复现
const RNG_SEED: [u8; 32] = [42; 32];

// ===========================================================================
// 辅助函数
// ===========================================================================

/// 生成唯一临时目录路径
fn temp_dir(suffix: &str) -> std::path::PathBuf {
    let nonce = rand::random::<u64>();
    std::env::temp_dir().join(format!("daoql-edu-bench-{suffix}-{nonce}"))
}

/// 清理并创建临时目录，返回路径
fn prepare_dir(path: &std::path::PathBuf) {
    let _ = std::fs::remove_dir_all(path);
    std::fs::create_dir_all(path).unwrap();
}

/// 生成随机浮点向量
fn random_vector(rng: &mut impl Rng, dim: usize) -> Vec<f32> {
    (0..dim).map(|_| rng.gen::<f32>()).collect()
}

/// 创建 DaoQL 实例（benchmark 模式下关闭 WAL fsync，排除磁盘 I/O 噪声）
fn open_for_bench(path: impl AsRef<std::path::Path>) -> DaoQL {
    let mut config = daoql_edu::Config::default();
    config.wal.sync_on_write = false;
    config.wal.flush_interval_ms = 3600_000; // 1 hour — 避免 benchmark 期间触发 flush
    config.wal.buffer_size = 64 * 1024 * 1024; // 64MB — 避免 buffer 满
    config.index_sync = false; // benchmark 跳过索引 fsync
    DaoQL::open_with_config(path, config).unwrap()
}

// ===========================================================================
// 1. Being 写入
// ===========================================================================

/// 批量写入 Being — 规模参数化
fn bench_write_being(c: &mut Criterion) {
    let mut group = c.benchmark_group("write_being");
    group.sampling_mode(SamplingMode::Auto);

    for size in [100usize, 1_000, 5_000] {
        group.throughput(Throughput::Elements(size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            &size,
            |b, &size| {
                let dir = temp_dir("write");
                prepare_dir(&dir);
                let daoql = open_for_bench(&dir);
                let mut counter = 0u64;

                // 预创建 beings（避免 benchmark 测量 `Being::new` 和字符串分配）
                let mut beings = Vec::with_capacity(size);
                for _ in 0..size {
                    beings.push(Being::new(format!("Being-{counter}"), "BenchmarkDef"));
                    counter += 1;
                }

                b.iter(|| {
                    black_box(daoql.write_batch(&beings).unwrap());
                });

                let _ = std::fs::remove_dir_all(&dir);
            },
        );
    }

    group.finish();
}

// ===========================================================================
// 2. 关系建立
// ===========================================================================

/// 链式关系建立 — 线性图 N 个节点、N-1 条边
fn bench_relate_chain(c: &mut Criterion) {
    let mut group = c.benchmark_group("relate_chain");
    group.sampling_mode(SamplingMode::Auto);

    for count in [100usize, 500, 1_000] {
        group.throughput(Throughput::Elements(count as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(count),
            &count,
            |b, &count| {
                let dir = temp_dir("relate");
                prepare_dir(&dir);
                let daoql = open_for_bench(&dir);

                // 预写入 N 个 Being
                let mut ids = Vec::with_capacity(count);
                for i in 0..count {
                    let being = Being::new(format!("Node-{i}"), "ChainNode");
                    ids.push(daoql.write(being).unwrap());
                }

                b.iter(|| {
                    for i in 0..ids.len().saturating_sub(1) {
                        daoql.relate(ids[i], ids[i + 1], 1).unwrap();
                        black_box(());
                    }
                });

                let _ = std::fs::remove_dir_all(&dir);
            },
        );
    }

    group.finish();
}

// ===========================================================================
// 3. 点查询
// ===========================================================================

/// 通过 BeingId 点查 — 数据库规模参数化
fn bench_point_query(c: &mut Criterion) {
    let mut group = c.benchmark_group("point_query");
    group.sampling_mode(SamplingMode::Auto);

    for db_size in [1_000usize, 5_000, 10_000] {
        group.throughput(Throughput::Elements(1));
        group.bench_with_input(
            BenchmarkId::from_parameter(db_size),
            &db_size,
            |b, &db_size| {
                let dir = temp_dir("point");
                prepare_dir(&dir);
                let daoql = open_for_bench(&dir);

                let mut ids = Vec::with_capacity(db_size);
                for i in 0..db_size {
                    let being = Being::new(format!("Target-{i}"), "QueryTarget");
                    ids.push(daoql.write(being).unwrap());
                }
                // 随机打乱查询顺序，避免缓存局部性偏差
                let mut rng = rand::rngs::StdRng::from_seed(RNG_SEED);
                ids.shuffle(&mut rng);
                let mut query_iter = ids.iter().cycle();

                b.iter(|| {
                    let id = *query_iter.next().unwrap();
                    black_box(daoql.query().being(id).execute().unwrap());
                });

                let _ = std::fs::remove_dir_all(&dir);
            },
        );
    }

    group.finish();
}

// ===========================================================================
// 4. 类型扫描
// ===========================================================================

/// 按 def 类型扫描 — 不同数据库规模和类型比例
fn bench_scan_by_def(c: &mut Criterion) {
    let mut group = c.benchmark_group("scan_by_def");
    group.sampling_mode(SamplingMode::Auto);

    let db_size = 5_000usize;
    let dir = temp_dir("scan");
    prepare_dir(&dir);
    let daoql = open_for_bench(&dir);

    // 混合类型：Person 50%, Order 30%, Product 20%
    let mut person_ids = Vec::new();
    for i in 0..db_size {
        let def = match i % 10 {
            0..=4 => {
                let being = Being::new(format!("Person-{i}"), "Person");
                person_ids.push(daoql.write(being).unwrap());
                continue;
            }
            5..=7 => "Order",
            _ => "Product",
        };
        let being = Being::new(format!("Item-{i}"), def);
        daoql.write(being).unwrap();
    }

    group.throughput(Throughput::Elements(person_ids.len() as u64));
    group.bench_function("scan_person_5k_mixed", |b| {
        b.iter(|| {
            black_box(daoql.query().scan("Person").execute().unwrap());
        });
    });

    group.finish();
    let _ = std::fs::remove_dir_all(&dir);
}

// ===========================================================================
// 5. 过滤扫描
// ===========================================================================

/// 带过滤条件的扫描 — 不同选择率
fn bench_scan_with_filter(c: &mut Criterion) {
    let mut group = c.benchmark_group("scan_with_filter");
    group.sampling_mode(SamplingMode::Auto);

    let db_size = 5_000usize;
    let dir = temp_dir("filter");
    prepare_dir(&dir);
    let daoql = open_for_bench(&dir);

    for i in 0..db_size {
        let weight = (i % 100) as f64;
        let mut being = Being::new(format!("Filtered-{i}"), "FilterDef");
        being.core.weight = weight;
        daoql.write(being).unwrap();
    }

    group.throughput(Throughput::Elements(db_size as u64));
    group.bench_function("filter_weight_gt_50", |b| {
        b.iter(|| {
            black_box(
                daoql
                    .query()
                    .scan("FilterDef")
                    .filter("weight", "gt", serde_json::json!(50.0))
                    .execute()
                    .unwrap(),
            );
        });
    });

    group.bench_function("filter_name_eq", |b| {
        b.iter(|| {
            black_box(
                daoql
                    .query()
                    .scan("FilterDef")
                    .filter("name", "eq", serde_json::json!("Filtered-2500"))
                    .execute()
                    .unwrap(),
            );
        });
    });

    group.finish();
    let _ = std::fs::remove_dir_all(&dir);
}

// ===========================================================================
// 5b. 聚合查询
// ===========================================================================

/// 列存聚合 — 不同数据规模
fn bench_aggregate(c: &mut Criterion) {
    let mut group = c.benchmark_group("aggregate");
    group.sampling_mode(SamplingMode::Auto);

    for db_size in [5_000usize, 10_000, 50_000, 100_000] {
        let dir = temp_dir("agg");
        prepare_dir(&dir);
        let daoql = open_for_bench(&dir);

        // 批量写入（避免逐条事务开销）
        let batch_size = 5000usize;
        for chunk_start in (0..db_size).step_by(batch_size) {
            let chunk_end = (chunk_start + batch_size).min(db_size);
            let mut batch = Vec::with_capacity(chunk_end - chunk_start);
            for i in chunk_start..chunk_end {
                let weight = (i % 100) as f64;
                let mut being = Being::new(format!("Agg-{i}"), "AggDef");
                being.core.weight = weight;
                batch.push(being);
            }
            daoql.write_batch(&batch).unwrap();
        }

        group.throughput(Throughput::Elements(db_size as u64));
        group.bench_with_input(
            BenchmarkId::new("sum_weight", db_size),
            &db_size,
            |b, _| {
                b.iter(|| {
                    black_box(
                        daoql
                            .query()
                            .scan("")
                            .aggregate("weight", daoql_edu::column::aggregation::AggregateOp::Sum)
                            .execute()
                            .unwrap(),
                    );
                });
            },
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    group.finish();
}

// ===========================================================================
// 5c. 混合查询（扫描 + 过滤 + 聚合）
// ===========================================================================

fn bench_mixed_query(c: &mut Criterion) {
    let mut group = c.benchmark_group("mixed_query");
    group.sampling_mode(SamplingMode::Auto);

    for db_size in [5_000usize, 100_000] {
        let dir = temp_dir("mixed");
        prepare_dir(&dir);
        let daoql = open_for_bench(&dir);

        // 批量写入
        let batch_size = 5000usize;
        for chunk_start in (0..db_size).step_by(batch_size) {
            let chunk_end = (chunk_start + batch_size).min(db_size);
            let mut batch = Vec::with_capacity(chunk_end - chunk_start);
            for i in chunk_start..chunk_end {
                let weight = (i % 100) as f64;
                let mut being = Being::new(format!("Mixed-{i}"), "MixedDef");
                being.core.weight = weight;
                batch.push(being);
            }
            daoql.write_batch(&batch).unwrap();
        }

        group.throughput(Throughput::Elements(db_size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(db_size),
            &db_size,
            |b, _| {
                b.iter(|| {
                    black_box(
                        daoql
                            .query()
                            .scan("")
                            .filter("weight", "gt", serde_json::json!(50.0))
                            .aggregate("weight", daoql_edu::column::aggregation::AggregateOp::Sum)
                            .execute()
                            .unwrap(),
                    );
                });
            },
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    group.finish();
}

// ===========================================================================
// 6. DSL 查询
// ===========================================================================

/// DSL 查询执行 — 完整解析+执行流水线
fn bench_dsl_query(c: &mut Criterion) {
    let mut group = c.benchmark_group("dsl_query");
    group.sampling_mode(SamplingMode::Auto);

    let db_size = 2_000usize;
    let dir = temp_dir("dsl");
    prepare_dir(&dir);
    let daoql = open_for_bench(&dir);

    for i in 0..db_size {
        let def = if i % 2 == 0 { "Material" } else { "Product" };
        let being = Being::new(format!("Item-{i}"), def);
        daoql.write(being).unwrap();
    }

    let dsl_scan = r#"query { Material { id, name } }"#;
    let dsl_limit = r#"query { Material(limit: 10) { id, name } }"#;

    group.throughput(Throughput::Elements(1));
    group.bench_function("dsl_scan_material", |b| {
        b.iter(|| {
            black_box(daoql.execute_dsl(dsl_scan).unwrap());
        });
    });

    group.bench_function("dsl_scan_limit_10", |b| {
        b.iter(|| {
            black_box(daoql.execute_dsl(dsl_limit).unwrap());
        });
    });

    group.finish();
    let _ = std::fs::remove_dir_all(&dir);
}

// ===========================================================================
// 7. 图遍历 — BFS
// ===========================================================================

/// BFS 遍历 — 星型图（1 中心 + N 子节点）
fn bench_graph_bfs(c: &mut Criterion) {
    let mut group = c.benchmark_group("graph_bfs");
    group.sampling_mode(SamplingMode::Auto);

    for fanout in [50usize, 100, 200] {
        group.throughput(Throughput::Elements((fanout + 1) as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(fanout),
            &fanout,
            |b, &fanout| {
                let dir = temp_dir("bfs");
                prepare_dir(&dir);
                let daoql = open_for_bench(&dir);

                let center = Being::new("Center", "Hub");
                let center_id = daoql.write(center).unwrap();
                let mut child_ids = Vec::with_capacity(fanout);
                for i in 0..fanout {
                    let child = Being::new(format!("Child-{i}"), "Leaf");
                    let child_id = daoql.write(child).unwrap();
                    child_ids.push(child_id);
                }
                for child_id in &child_ids {
                    daoql.relate(center_id, *child_id, 1).unwrap();
                }

                let center_offset = daoql
                    .graph
                    .borrow()
                    .find_node_offset(center_id)
                    .unwrap();

                b.iter(|| {
                    let mut graph = daoql.graph.borrow_mut();
                    black_box(
                        bfs(&mut graph, center_offset, 2, Some(&EdgeFilter::by_type(1)))
                            .unwrap(),
                    );
                });

                let _ = std::fs::remove_dir_all(&dir);
            },
        );
    }

    group.finish();
}

// ===========================================================================
// 7b. 图遍历 — BFS 大规模（1,365 nodes depth=5，四叉树拓扑）
// ===========================================================================

fn bench_graph_bfs_large(c: &mut Criterion) {
    let mut group = c.benchmark_group("graph_bfs_large");
    group.sampling_mode(SamplingMode::Auto);

    let dir = temp_dir("bfs_large");
    prepare_dir(&dir);
    let daoql = open_for_bench(&dir);

    // 构建四叉树：root → 4 children → 16 grandchildren → ... → depth=5
    let mut all_ids = Vec::new();
    let root = Being::new("Root", "TreeNode");
    let root_id = daoql.write(root).unwrap();
    all_ids.push(root_id);

    let mut prev_level = vec![root_id];
    for _depth in 0..5 {
        let mut next_level = Vec::new();
        for parent_id in &prev_level {
            for _ in 0..4 {
                let child = Being::new(format!("Node-{}-{}", _depth, all_ids.len()), "TreeNode");
                let child_id = daoql.write(child).unwrap();
                daoql.relate(*parent_id, child_id, 1).unwrap();
                next_level.push(child_id);
                all_ids.push(child_id);
            }
        }
        prev_level = next_level;
    }

    let total_nodes = all_ids.len() as u64;
    group.throughput(Throughput::Elements(total_nodes));

    let root_offset = daoql.graph.borrow().find_node_offset(root_id).unwrap();

    group.bench_function("bfs_1365_nodes_depth5", |b| {
        b.iter(|| {
            let mut graph = daoql.graph.borrow_mut();
            black_box(
                daoql_edu::graph::bfs(&mut graph, root_offset, 5, Some(&daoql_edu::graph::EdgeFilter::by_type(1)))
                    .unwrap(),
            );
        });
    });

    group.finish();
    let _ = std::fs::remove_dir_all(&dir);
}

// ===========================================================================
// 8. 图遍历 — DFS
// ===========================================================================

/// DFS 遍历 — 链式图（N 个节点线性连接）
fn bench_graph_dfs(c: &mut Criterion) {
    let mut group = c.benchmark_group("graph_dfs");
    group.sampling_mode(SamplingMode::Auto);

    for chain_len in [50usize, 100, 200] {
        group.throughput(Throughput::Elements(chain_len as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(chain_len),
            &chain_len,
            |b, &chain_len| {
                let dir = temp_dir("dfs");
                prepare_dir(&dir);
                let daoql = open_for_bench(&dir);

                let mut ids = Vec::with_capacity(chain_len);
                for i in 0..chain_len {
                    let node = Being::new(format!("Chain-{i}"), "ChainNode");
                    ids.push(daoql.write(node).unwrap());
                }
                for i in 0..ids.len().saturating_sub(1) {
                    daoql.relate(ids[i], ids[i + 1], 1).unwrap();
                }

                let start_offset = daoql
                    .graph
                    .borrow()
                    .find_node_offset(ids[0])
                    .unwrap();

                b.iter(|| {
                    let mut graph = daoql.graph.borrow_mut();
                    black_box(
                        dfs(&mut graph, start_offset, chain_len, Some(&EdgeFilter::by_type(1)))
                            .unwrap(),
                    );
                });

                let _ = std::fs::remove_dir_all(&dir);
            },
        );
    }

    group.finish();
}

// ===========================================================================
// 9. 向量索引 — 批量插入
// ===========================================================================

/// HNSW 向量索引批量插入
fn bench_vector_insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("vector_insert");
    group.sampling_mode(SamplingMode::Auto);

    for count in [500usize, 1_000, 2_000] {
        group.throughput(Throughput::Elements(count as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(count),
            &count,
            |b, &count| {
                let mut rng = rand::rngs::StdRng::from_seed(RNG_SEED);

                b.iter(|| {
                    let mut index = HnswIndex::new(DIM, 16, 200, 50);
                    for _ in 0..count {
                        let vec = random_vector(&mut rng, DIM);
                        let id = BeingId::new();
                        index.insert(id, vec).unwrap();
                        black_box(());
                    }
                });
            },
        );
    }

    group.finish();
}

// ===========================================================================
// 10. 向量索引 — 近似最近邻搜索
// ===========================================================================

/// HNSW 近似最近邻搜索 — 不同索引规模和 k 值
fn bench_vector_search(c: &mut Criterion) {
    let mut group = c.benchmark_group("vector_search");
    group.sampling_mode(SamplingMode::Auto);

    let count = 10_000usize;
    let mut rng = rand::rngs::StdRng::from_seed(RNG_SEED);
    let mut index = HnswIndex::new(DIM, 16, 100, 64);

    // 批量插入以加速 setup
    let mut batch = Vec::with_capacity(count);
    for _ in 0..count {
        let vec = random_vector(&mut rng, DIM);
        let id = BeingId::new();
        batch.push((id, vec));
    }
    index.insert_batch(&batch).unwrap();

    for k in [1usize, 5, 10, 20] {
        group.throughput(Throughput::Elements(k as u64));
        group.bench_with_input(BenchmarkId::from_parameter(k), &k, |b, &k| {
            let query = random_vector(&mut rng, DIM);
            b.iter(|| {
                black_box(index.search(&query, k).unwrap());
            });
        });
    }

    group.finish();
}

// ===========================================================================
// Criterion 分组注册
// ===========================================================================

criterion_group!(
    benches,
    bench_write_being,
    bench_relate_chain,
    bench_point_query,
    bench_scan_by_def,
    bench_scan_with_filter,
    bench_aggregate,
    bench_mixed_query,
    bench_dsl_query,
    bench_graph_bfs,
    bench_graph_bfs_large,
    bench_graph_dfs,
    bench_vector_insert,
    bench_vector_search,
);
criterion_main!(benches);
