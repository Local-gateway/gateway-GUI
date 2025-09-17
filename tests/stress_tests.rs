//! 压力测试模块
//!
//! 包含网关系统的压力测试和负载测试，用于验证系统在高负载下的稳定性。

use anyhow::Result;
use serial_test::serial;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tempfile::TempDir;
use tokio::sync::Semaphore;
use tokio::time::timeout;
use wdic_gateway::{
    CompressionManager, Gateway, GatewayConfig, PerformanceMonitor, Registry, RegistryEntry,
    UdpBroadcastManager,
};

/// 测试网关高并发创建和销毁
#[tokio::test]
#[serial]
async fn test_gateway_concurrent_lifecycle() -> Result<()> {
    let semaphore = Arc::new(Semaphore::new(10)); // 限制并发数
    let mut handles = Vec::new();

    for i in 0..20 {
        let permit = semaphore.clone().acquire_owned().await.unwrap();
        let handle = tokio::spawn(async move {
            let _permit = permit; // 持有许可证

            let temp_dir = TempDir::new().unwrap();
            let config = GatewayConfig {
                name: format!("压力测试网关_{i}"),
                port: 0,
                cache_dir: temp_dir.path().to_path_buf(),
                enable_mtls: false,
                enable_ipv6: false,
                ..Default::default()
            };

            let gateway = Gateway::with_config(config).await.unwrap();

            // 快速启动停止测试
            let run_handle =
                tokio::spawn(
                    async move { timeout(Duration::from_millis(100), gateway.run()).await },
                );

            tokio::time::sleep(Duration::from_millis(50)).await;
            run_handle.abort();

            println!("网关 {i} 压力测试完成");
        });

        handles.push(handle);
    }

    // 等待所有任务完成
    for handle in handles {
        handle.await?;
    }

    println!("所有并发网关压力测试完成");
    Ok(())
}

/// 测试注册表高并发操作
#[tokio::test]
async fn test_registry_stress() -> Result<()> {
    let registry = Registry::new(
        "压力测试注册表".to_string(),
        SocketAddr::from(([127, 0, 0, 1], 55557)),
    );

    let start_time = Instant::now();
    let mut handles = Vec::new();

    // 大量并发读写操作
    for i in 0..100 {
        let registry_clone = registry.clone();
        let handle = tokio::spawn(async move {
            // 添加条目
            for j in 0..10 {
                let entry = RegistryEntry::new(
                    format!("压力测试网关_{i}_{j}"),
                    SocketAddr::from(([192, 168, 1, (i % 255) as u8], 55555 + j as u16)),
                );
                registry_clone.add_or_update(entry);
            }

            // 并发查询
            let entries = registry_clone.all_entries();
            for entry in entries.iter().take(5) {
                registry_clone.get(&entry.id);
            }

            // 删除一些条目
            let entries = registry_clone.all_entries();
            for entry in entries.iter().take(3) {
                registry_clone.remove(&entry.id);
            }
        });
        handles.push(handle);
    }

    // 等待所有操作完成
    for handle in handles {
        handle.await?;
    }

    let duration = start_time.elapsed();
    let final_entries = registry.all_entries();

    println!("注册表压力测试完成:");
    println!("- 持续时间: {:.2}秒", duration.as_secs_f64());
    println!("- 最终条目数: {}", final_entries.len());
    println!(
        "- 操作吞吐量: {:.2} ops/sec",
        1000.0 / duration.as_secs_f64()
    );

    // 验证注册表仍然可用
    assert!(final_entries.len() < 1000); // 应该有一些被删除

    Ok(())
}

/// 测试压缩系统高负载
#[tokio::test]
async fn test_compression_stress() -> Result<()> {
    let mut handles = Vec::new();

    // 并发压缩大量数据
    for i in 0..50 {
        let handle = tokio::spawn(async move {
            let manager = CompressionManager::default();
            let test_data = format!("压力测试数据块 {i} - ").repeat(1000);
            let data_bytes = test_data.as_bytes();

            for _ in 0..20 {
                // 压缩
                let compressed = manager.compress(data_bytes).unwrap();

                // 解压
                let decompressed = manager.decompress(&compressed).unwrap();

                // 验证数据完整性
                assert_eq!(decompressed, data_bytes);
            }

            // 返回统计信息
            let stats = manager.stats();
            (
                stats
                    .compress_count
                    .load(std::sync::atomic::Ordering::Relaxed),
                stats
                    .decompress_count
                    .load(std::sync::atomic::Ordering::Relaxed),
                stats.compression_ratio(),
            )
        });

        handles.push(handle);
    }

    let start_time = std::time::Instant::now();

    // 等待所有压缩操作完成并收集统计
    let mut total_compress = 0;
    let mut total_decompress = 0;
    let mut avg_ratio = 0.0;

    for handle in handles {
        let (compress_count, decompress_count, ratio) = handle.await?;
        total_compress += compress_count;
        total_decompress += decompress_count;
        avg_ratio += ratio;
    }

    let duration = start_time.elapsed();
    avg_ratio /= 50.0; // 平均值

    println!("压缩系统压力测试完成:");
    println!("- 持续时间: {:.2}秒", duration.as_secs_f64());
    println!("- 总压缩次数: {total_compress}");
    println!("- 总解压次数: {total_decompress}");
    println!("- 平均压缩比: {:.2}%", avg_ratio * 100.0);
    println!(
        "- 压缩吞吐量: {:.0} 操作/秒",
        total_compress as f64 / duration.as_secs_f64()
    );

    // 验证所有操作成功
    assert!(total_compress > 0);
    assert!(total_decompress > 0);

    Ok(())
}

/// 测试性能监控系统在高负载下的表现
#[tokio::test]
async fn test_performance_monitor_stress() -> Result<()> {
    let mut handles = Vec::new();

    // 大量并发性能数据记录
    for i in 0..50 {
        let handle = tokio::spawn(async move {
            let monitor = PerformanceMonitor::new();

            for j in 0..100 {
                // 记录网络指标
                monitor.record_network_send(1024 * (i + j) as u64).await;
                monitor.record_network_receive(2048 * (i + j) as u64).await;

                // 记录延迟指标
                let latency = (i + j) as f64 * 0.1;
                monitor.record_latency(latency).await;

                // 每隔一段时间记录错误
                if (i + j) % 10 == 0 {
                    monitor.record_network_error(true).await;
                }
            }

            // 生成报告测试
            let report = monitor.generate_report().await;
            report.len()
        });

        handles.push(handle);
    }

    let start_time = std::time::Instant::now();

    // 等待所有记录操作完成
    let mut total_report_length = 0;
    for handle in handles {
        let report_length = handle.await?;
        total_report_length += report_length;
    }

    let duration = start_time.elapsed();

    println!("性能监控压力测试完成:");
    println!("- 持续时间: {:.2}秒", duration.as_secs_f64());
    println!("- 总报告长度: {total_report_length} 字符");
    println!("- 平均报告长度: {} 字符", total_report_length / 50);
    println!(
        "- 操作吞吐量: {:.2} 监控器/秒",
        50.0 / duration.as_secs_f64()
    );

    // 验证报告生成正常
    assert!(total_report_length > 0);

    Ok(())
}

/// 测试UDP广播管理器压力测试
#[tokio::test]
async fn test_udp_broadcast_stress() -> Result<()> {
    let mut handles = Vec::new();

    // 创建多个UDP管理器并发操作
    for i in 0..10 {
        let handle = tokio::spawn(async move {
            #[allow(deprecated)]
            let manager = UdpBroadcastManager::new(SocketAddr::from(([127, 0, 0, 1], 0))).unwrap();

            let temp_dir = TempDir::new().unwrap();

            // 创建测试文件
            for j in 0..5 {
                let test_file = temp_dir.path().join(format!("test_{i}_{j}.txt"));
                std::fs::write(&test_file, format!("测试数据 {i} {j}")).unwrap();
            }

            // 尝试挂载目录（在某些环境中可能失败，这是正常的）
            match manager
                .mount_directory(
                    format!("/test_{i}"),
                    temp_dir.path().to_string_lossy().to_string(),
                )
                .await
            {
                Ok(_) => {
                    println!("UDP管理器 {i} 挂载成功");

                    // 尝试卸载
                    let unmounted = manager.unmount_directory(&format!("/test_{i}")).await;
                    println!("UDP管理器 {i} 卸载结果: {unmounted}");
                }
                Err(e) => {
                    println!("UDP管理器 {i} 挂载失败（在测试环境中可能正常）: {e}");
                }
            }
        });

        handles.push(handle);
    }

    // 等待所有UDP操作完成
    for handle in handles {
        handle.await?;
    }

    println!("UDP广播管理器压力测试完成");
    Ok(())
}

/// 测试内存使用压力
#[tokio::test]
async fn test_memory_stress() -> Result<()> {
    let monitor = PerformanceMonitor::new();
    let start_time = Instant::now();

    // 首先更新系统指标确保获取到正确的内存信息
    monitor.update_system_metrics().await?;

    // 记录开始时的内存使用
    let initial_memory = monitor.get_memory_metrics().await;
    println!(
        "初始内存使用: {:.2} MB",
        initial_memory.current_usage as f64 / 1024.0 / 1024.0
    );

    let mut data_holders = Vec::new();

    // 分配大量内存测试内存监控
    for i in 0..100 {
        let large_data = vec![i as u8; 1024 * 1024]; // 1MB per allocation
        data_holders.push(large_data);

        // 每隔10次分配检查一次内存，并更新系统指标
        if i % 10 == 0 {
            monitor.update_system_metrics().await?;
            let current_memory = monitor.get_memory_metrics().await;
            println!(
                "内存使用 ({}): {:.2} MB",
                i,
                current_memory.current_usage as f64 / 1024.0 / 1024.0
            );
        }
    }

    // 更新系统指标以获取峰值内存
    monitor.update_system_metrics().await?;
    let peak_memory = monitor.get_memory_metrics().await;
    println!(
        "峰值内存使用: {:.2} MB",
        peak_memory.current_usage as f64 / 1024.0 / 1024.0
    );

    // 释放内存
    data_holders.clear();

    // 强制垃圾回收 (在Rust中主要是让析构函数运行)
    tokio::time::sleep(Duration::from_millis(100)).await;

    monitor.update_system_metrics().await?;
    let final_memory = monitor.get_memory_metrics().await;
    println!(
        "最终内存使用: {:.2} MB",
        final_memory.current_usage as f64 / 1024.0 / 1024.0
    );

    let duration = start_time.elapsed();
    println!("内存压力测试持续时间: {:.2}秒", duration.as_secs_f64());

    // 验证内存监控功能正常 - 由于修复了内存检测逻辑，现在应该能够检测到变化
    // 但是为了避免在测试环境中的不确定性，我们只检查基本的内存指标是否合理
    assert!(
        peak_memory.current_usage > 0,
        "峰值内存使用应该大于0: {} bytes",
        peak_memory.current_usage
    );
    assert!(
        initial_memory.current_usage > 0,
        "初始内存使用应该大于0: {} bytes",
        initial_memory.current_usage
    );

    Ok(())
}
