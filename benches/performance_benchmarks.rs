//! 网关性能基准测试
//!
//! 测试网关各个组件的性能指标，包括内存使用、吞吐量、延迟等关键指标。

use criterion::{criterion_group, criterion_main, Criterion, black_box, BenchmarkId, Throughput};
use std::time::Duration;
use tokio::runtime::Runtime;
use wdic_gateway::{
    UdpBroadcastManager, 
    UdpToken, 
    DirectoryIndex,
    DirectoryEntry,
    PerformanceMonitor
};
use uuid::Uuid;

/// 测试序列化性能
fn bench_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("序列化性能测试");
    
    // 测试不同大小的 UDP 令牌序列化
    for size in [10, 100, 1000].iter() {
        let keywords: Vec<String> = (0..*size).map(|i| format!("keyword{}", i)).collect();
        let token = UdpToken::DirectorySearch {
            searcher_id: Uuid::new_v4(),
            keywords: keywords.into(),
            search_id: Uuid::new_v4(),
        };
        
        group.throughput(Throughput::Elements(*size as u64));
        
        group.bench_with_input(
            BenchmarkId::new("serde_json序列化", size),
            &token,
            |b, token| {
                b.iter(|| {
                    black_box(serde_json::to_string(token).unwrap());
                });
            },
        );
    }
    
    group.finish();
}

/// 测试反序列化性能
fn bench_deserialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("反序列化性能测试");
    
    for size in [10, 100, 1000].iter() {
        let keywords: Vec<String> = (0..*size).map(|i| format!("keyword{}", i)).collect();
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
            path: format!("file{}.txt", i),
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
    let rt = Runtime::new().unwrap();
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
                    content: format!("压力测试消息 {}", i),
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
                path: format!("stress_file_{}.dat", i),
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

criterion_group!(
    benches,
    bench_serialization,
    bench_deserialization,
    bench_directory_operations,
    bench_network_operations,
    bench_memory_usage,
    bench_stress_testing
);

criterion_main!(benches);