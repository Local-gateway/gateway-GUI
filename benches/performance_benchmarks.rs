//! 网关性能基准测试
//!
//! 测试网关各个组件的性能指标，包括内存使用、吞吐量、延迟等关键指标。
//! 包含zstd压缩和lock-free并发性能测试。

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::hint::black_box;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::runtime::Runtime;
use uuid::Uuid;
use wdic_gateway::{
    CompressionConfig, CompressionManager, DirectoryEntry, DirectoryIndex, PerformanceMonitor,
    Registry, RegistryEntry, UdpToken,
};

/// 测试序列化性能
fn bench_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("序列化性能测试");
    group.sample_size(200);
    group.measurement_time(Duration::from_secs(15));

    // 测试不同大小的 UDP 令牌序列化
    for size in [10, 100, 1000].iter() {
        let keywords: Vec<String> = (0..*size).map(|i| format!("keyword{i}")).collect();
        let token = UdpToken::DirectorySearch {
            searcher_id: Uuid::new_v4(),
            keywords: keywords.into(),
            search_id: Uuid::new_v4(),
        };

        // 计算序列化后的字节大小
        let serialized_size = serde_json::to_string(&token).unwrap().len();
        group.throughput(Throughput::Bytes(serialized_size as u64));

        group.bench_with_input(
            BenchmarkId::new("serde_json序列化", size),
            &token,
            |b, token| {
                b.iter_batched(
                    || token.clone(),
                    |token| {
                        let result = black_box(serde_json::to_string(&token).unwrap());
                        // 输出性能数据
                        if !result.is_empty() {
                            black_box(result.len());
                        }
                        result
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

/// 测试反序列化性能
fn bench_deserialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("反序列化性能测试");

    for size in [10, 100, 1000].iter() {
        let keywords: Vec<String> = (0..*size).map(|i| format!("keyword{i}")).collect();
        let token = UdpToken::DirectorySearch {
            searcher_id: Uuid::new_v4(),
            keywords: keywords.into(),
            search_id: Uuid::new_v4(),
        };

        let json_data = serde_json::to_string(&token).unwrap();

        group.throughput(Throughput::Elements(*size as u64));

        group.bench_with_input(
            BenchmarkId::new("serde_json反序列化", size),
            &json_data,
            |b, data| {
                b.iter(|| {
                    let _: UdpToken = black_box(serde_json::from_str(data).unwrap());
                });
            },
        );
    }

    group.finish();
}

/// 测试目录索引性能
fn bench_directory_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("目录操作性能测试");

    // 创建测试目录索引 - 使用一个简单的手动构建方式
    let mut index = DirectoryIndex {
        root_path: "/tmp/test".to_string(),
        entries: Vec::new(),
        generated_at: chrono::Utc::now(),
    };

    for i in 0..1000 {
        index.entries.push(DirectoryEntry {
            path: format!("file{i}.txt"),
            size: i * 1024,
            is_dir: false,
            modified: chrono::Utc::now(),
        });
    }

    let keywords = vec!["file".to_string(), "txt".to_string()];
    group.bench_function("目录搜索", |b| {
        b.iter(|| {
            black_box(index.search(&keywords));
        });
    });

    group.finish();
}

/// 测试网络操作性能
fn bench_network_operations(c: &mut Criterion) {
    let _rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("网络操作性能测试");
    group.sample_size(50); // 减少样本数量，因为网络操作比较慢

    // 测试令牌广播
    let token = UdpToken::InfoMessage {
        sender_id: Uuid::new_v4(),
        content: "性能测试消息".to_string(),
        message_id: Uuid::new_v4(),
    };

    group.bench_function("令牌序列化广播准备", |b| {
        b.iter(|| {
            black_box(serde_json::to_string(&token).unwrap());
        });
    });

    group.finish();
}

/// 测试内存使用性能
fn bench_memory_usage(c: &mut Criterion) {
    let mut group = c.benchmark_group("内存使用性能测试");

    group.bench_function("性能监控器创建", |b| {
        b.iter(|| {
            black_box(PerformanceMonitor::new());
        });
    });

    group.finish();
}

/// 测试极限性能情况
fn bench_stress_testing(c: &mut Criterion) {
    let mut group = c.benchmark_group("压力测试");
    group.sample_size(20);
    group.measurement_time(Duration::from_secs(5));

    group.bench_function("大量令牌序列化", |b| {
        b.iter(|| {
            let mut tokens = Vec::new();
            for i in 0..1000 {
                let token = UdpToken::InfoMessage {
                    sender_id: Uuid::new_v4(),
                    content: format!("压力测试消息 {i}"),
                    message_id: Uuid::new_v4(),
                };
                tokens.push(black_box(serde_json::to_string(&token).unwrap()));
            }
            black_box(tokens);
        });
    });

    group.bench_function("大量目录搜索", |b| {
        let mut index = DirectoryIndex {
            root_path: "/tmp/stress".to_string(),
            entries: Vec::new(),
            generated_at: chrono::Utc::now(),
        };

        for i in 0..10000 {
            index.entries.push(DirectoryEntry {
                path: format!("stress_file_{i}.dat"),
                size: i * 512,
                is_dir: false,
                modified: chrono::Utc::now(),
            });
        }

        b.iter(|| {
            for i in 0..100 {
                let keywords = vec![format!("stress_file_{}", i * 10)];
                black_box(index.search(&keywords));
            }
        });
    });

    group.finish();
}

/// 测试zstd压缩性能
fn bench_compression_performance(c: &mut Criterion) {
    let mut group = c.benchmark_group("zstd压缩性能测试");

    // 创建不同压缩级别的管理器
    let configs = vec![
        (1, "快速压缩"),
        (3, "默认压缩"),
        (6, "高压缩比"),
        (9, "最高压缩比"),
    ];

    // 测试不同大小的数据
    for size in [1024, 4096, 16384, 65536].iter() {
        let data = "x".repeat(*size).into_bytes();
        group.throughput(Throughput::Bytes(*size as u64));

        for (level, name) in &configs {
            let config = CompressionConfig {
                level: *level,
                min_compress_size: 0, // 强制压缩所有数据
                max_chunk_size: 1024 * 1024,
                enable_dict: false,
            };
            let manager = CompressionManager::new(config);

            group.bench_with_input(
                BenchmarkId::new(format!("压缩-{name}"), size),
                &data,
                |b, data| {
                    b.iter(|| {
                        black_box(manager.compress(data).unwrap());
                    });
                },
            );

            // 先压缩数据用于解压测试
            let compressed_data = manager.compress(&data).unwrap();
            group.bench_with_input(
                BenchmarkId::new(format!("解压-{name}"), size),
                &compressed_data,
                |b, compressed| {
                    b.iter(|| {
                        black_box(manager.decompress(compressed).unwrap());
                    });
                },
            );
        }
    }

    group.finish();
}

/// 测试lock-free注册表性能
fn bench_lockfree_registry(c: &mut Criterion) {
    let mut group = c.benchmark_group("lock-free注册表性能测试");

    let registry = Registry::new(
        "test_gateway".to_string(),
        SocketAddr::from(([127, 0, 0, 1], 55555)),
    );

    // 预填充一些数据
    for i in 0..1000 {
        let entry = RegistryEntry::new(
            format!("gateway_{i}"),
            SocketAddr::from(([192, 168, 1, (i % 255) as u8], 55555 + (i % 1000) as u16)),
        );
        registry.add_or_update(entry);
    }

    group.bench_function("并发添加操作", |b| {
        b.iter(|| {
            for i in 0..100 {
                let entry = RegistryEntry::new(
                    format!("bench_gateway_{i}"),
                    SocketAddr::from(([10, 0, 0, (i % 255) as u8], 55555 + (i % 1000) as u16)),
                );
                black_box(registry.add_or_update(entry));
            }
        });
    });

    group.bench_function("并发查询操作", |b| {
        let entries = registry.all_entries();
        b.iter(|| {
            for entry in &entries[..std::cmp::min(100, entries.len())] {
                black_box(registry.get(&entry.id));
            }
        });
    });

    group.bench_function("获取所有条目", |b| {
        b.iter(|| {
            black_box(registry.all_entries());
        });
    });

    group.bench_function("清理过期条目", |b| {
        b.iter(|| {
            black_box(registry.cleanup_expired(3600));
        });
    });

    group.finish();
}

/// 测试压缩比对比
fn bench_compression_ratio(c: &mut Criterion) {
    let mut group = c.benchmark_group("压缩比对比测试");
    group.sample_size(50);
    group.measurement_time(Duration::from_secs(20));

    // 创建全局压缩管理器用于统计
    let global_manager = CompressionManager::default();

    // 测试不同类型的数据
    let test_data = vec![
        (
            "JSON数据",
            serde_json::to_string(&UdpToken::InfoMessage {
                sender_id: Uuid::new_v4(),
                content: "这是一个测试消息，包含一些重复的内容和结构化数据".repeat(20),
                message_id: Uuid::new_v4(),
            })
            .unwrap()
            .into_bytes(),
        ),
        ("重复文本", "重复的文本内容".repeat(100).into_bytes()),
        ("随机数据", (0..1000).map(|i| (i * 17 + 7) as u8).collect()),
        ("二进制数据", vec![0u8; 1000]),
    ];

    for (data_type, data) in test_data {
        let manager = CompressionManager::default();
        let original_size = data.len();

        group.throughput(Throughput::Bytes(original_size as u64));

        group.bench_function(format!("压缩-{data_type}"), |b| {
            b.iter_batched(
                || data.clone(),
                |data| {
                    let compressed = black_box(manager.compress(&data).unwrap());
                    let ratio = compressed.len() as f64 / data.len() as f64;

                    // 输出压缩性能数据
                    println!(
                        "压缩数据类型: {}, 原始大小: {} 字节, 压缩后: {} 字节, 压缩比: {:.2}%",
                        data_type,
                        data.len(),
                        compressed.len(),
                        ratio * 100.0
                    );

                    black_box((compressed, ratio));
                },
                criterion::BatchSize::SmallInput,
            );
        });

        // 测试解压性能
        let compressed_data = manager.compress(&data).unwrap();
        group.bench_function(format!("解压-{data_type}"), |b| {
            b.iter_batched(
                || compressed_data.clone(),
                |compressed| {
                    let decompressed = black_box(manager.decompress(&compressed).unwrap());
                    assert_eq!(decompressed.len(), original_size);
                    decompressed
                },
                criterion::BatchSize::SmallInput,
            );
        });

        // 更新全局统计
        global_manager.compress(&data).unwrap();
        global_manager.decompress(&compressed_data).unwrap();
    }

    // 显示压缩统计
    let stats = global_manager.stats();
    println!(
        "压缩统计: 总压缩次数: {}, 总解压次数: {}, 平均压缩比: {:.2}%",
        stats
            .compress_count
            .load(std::sync::atomic::Ordering::Relaxed),
        stats
            .decompress_count
            .load(std::sync::atomic::Ordering::Relaxed),
        stats.compression_ratio() * 100.0
    );

    group.finish();
}

/// 配置基准测试参数以显示性能数据
fn configure_criterion() -> Criterion {
    Criterion::default()
        .with_output_color(true)
        .measurement_time(Duration::from_secs(10))
        .warm_up_time(Duration::from_secs(3))
        .sample_size(100)
        .significance_level(0.02)
        .noise_threshold(0.05)
}

criterion_group!(
    name = benches;
    config = configure_criterion();
    targets = bench_serialization,
    bench_deserialization,
    bench_directory_operations,
    bench_network_operations,
    bench_memory_usage,
    bench_stress_testing,
    bench_compression_performance,
    bench_lockfree_registry,
    bench_compression_ratio
);

criterion_main!(benches);
