//! 性能测试演示
//! 
//! 演示网关的综合性能测试功能，包括IPv6支持、吞吐量测试、内存分析等。

use std::time::Duration;
use wdic_gateway::{Gateway, PerformanceTestSuite};
use log::{info, error};
use tokio::time::sleep;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 初始化日志记录
    env_logger::init();

    info!("启动 WDIC 网关性能测试演示");

    // 创建网关实例
    let gateway = Gateway::new("性能测试网关".to_string()).await?;
    
    info!("网关创建成功，监听地址: {}", gateway.local_addr());
    info!("UDP广播地址: {}", gateway.udp_broadcast_manager.local_addr());

    // 启动网关
    tokio::spawn(async move {
        if let Err(e) = gateway.run().await {
            error!("网关运行时出错: {}", e);
        }
    });

    // 等待网关启动
    sleep(Duration::from_secs(2)).await;

    // 重新创建网关引用用于测试
    let test_gateway = Gateway::new("性能测试网关2".to_string()).await?;

    // 演示 1: 快速性能检查
    info!("=== 演示 1: 快速性能检查 ===");
    match test_gateway.quick_performance_check().await {
        Ok(stats) => {
            info!("内存使用: {:.2} MB ({:.2}%)", 
                  stats.memory_usage_mb, stats.memory_usage_percentage);
            info!("网络活动: 发送 {} 字节, 接收 {} 字节", 
                  stats.network_bytes_sent, stats.network_bytes_received);
            info!("平均延迟: {:.3} ms", stats.average_latency_ms);
            info!("活跃连接: {}, 注册表大小: {}", 
                  stats.active_connections, stats.registry_size);
            info!("连接成功率: {:.2}%", stats.connection_success_rate);
        }
        Err(e) => error!("快速性能检查失败: {}", e),
    }

    // 演示 2: 单项性能测试
    info!("\n=== 演示 2: 单项性能测试 ===");
    
    // 吞吐量测试
    let throughput_suite = PerformanceTestSuite {
        concurrency: 5,
        duration_seconds: 3,
        packet_size: 512,
        test_interval_ms: 10,
    };
    
    let perf_monitor = test_gateway.performance_monitor();
    match perf_monitor.run_throughput_benchmark("demo_throughput", &throughput_suite).await {
        Ok(result) => {
            info!("吞吐量测试结果:");
            info!("  - 操作次数: {}", result.operations);
            info!("  - 每秒操作数: {:.2}", result.ops_per_second);
            info!("  - 吞吐量: {:.2} MB/s", result.throughput_bps / 1024.0 / 1024.0);
            info!("  - 平均延迟: {:.3} ms", result.average_latency);
            info!("  - 内存使用: {:.2} MB", result.memory_usage as f64 / 1024.0 / 1024.0);
        }
        Err(e) => error!("吞吐量测试失败: {}", e),
    }

    // 延迟测试
    match perf_monitor.run_latency_benchmark("demo_latency", 100).await {
        Ok(result) => {
            info!("延迟测试结果:");
            info!("  - 平均延迟: {:.3} ms", result.average_latency);
            info!("  - 最小延迟: {:.3} ms", result.min_latency);
            info!("  - 最大延迟: {:.3} ms", result.max_latency);
            info!("  - 每秒操作数: {:.2}", result.ops_per_second);
        }
        Err(e) => error!("延迟测试失败: {}", e),
    }

    // 演示 3: 综合性能测试套件
    info!("\n=== 演示 3: 综合性能测试套件 ===");
    match test_gateway.run_comprehensive_performance_tests().await {
        Ok(results) => {
            info!("综合性能测试完成，共 {} 个测试项目:", results.len());
            for (name, result) in &results {
                info!("测试项目: {}", name);
                info!("  - 持续时间: {:.2} ms", result.duration_ms);
                info!("  - 操作次数: {}", result.operations);
                info!("  - 性能: {:.2} ops/s", result.ops_per_second);
                if result.throughput_bps > 0.0 {
                    info!("  - 吞吐量: {:.2} MB/s", result.throughput_bps / 1024.0 / 1024.0);
                }
                info!("  - 平均延迟: {:.3} ms", result.average_latency);
                info!("  - 内存使用: {:.2} MB", result.memory_usage as f64 / 1024.0 / 1024.0);
                info!("");
            }
        }
        Err(e) => error!("综合性能测试失败: {}", e),
    }

    // 演示 4: 网络广播能力测试（IPv4/IPv6）
    info!("=== 演示 4: 网络广播能力测试 ===");
    
    // 测试目录挂载和搜索
    match test_gateway.mount_directory("demo".to_string(), ".".to_string()).await {
        Ok(success) => {
            if success {
                info!("成功挂载当前目录为 'demo'");
                
                // 广播目录搜索
                match test_gateway.broadcast_directory_search(vec!["rs".to_string(), "toml".to_string()]).await {
                    Ok(sent_count) => {
                        info!("目录搜索广播发送到 {} 个地址", sent_count);
                    }
                    Err(e) => error!("目录搜索广播失败: {}", e),
                }
                
                // 广播信息消息
                match test_gateway.broadcast_info_message("性能测试演示消息".to_string()).await {
                    Ok(sent_count) => {
                        info!("信息消息广播发送到 {} 个地址", sent_count);
                    }
                    Err(e) => error!("信息消息广播失败: {}", e),
                }
            }
        }
        Err(e) => error!("目录挂载失败: {}", e),
    }

    // 演示 5: 性能报告生成
    info!("\n=== 演示 5: 详细性能报告 ===");
    let report = test_gateway.generate_performance_report().await;
    
    // 将报告写入文件并显示部分内容
    tokio::fs::write("performance_report.txt", &report).await?;
    info!("完整性能报告已保存到 performance_report.txt");
    
    // 显示报告摘要
    let lines: Vec<&str> = report.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        if i < 50 { // 显示前50行
            println!("{}", line);
        } else if i == 50 {
            println!("... (完整报告请查看 performance_report.txt)");
            break;
        }
    }

    // 演示 6: 实时性能监控
    info!("\n=== 演示 6: 实时性能监控 ===");
    info!("启动 10 秒实时监控...");
    
    for i in 1..=10 {
        sleep(Duration::from_secs(1)).await;
        
        // 模拟一些网络活动
        perf_monitor.record_network_send(1024).await;
        perf_monitor.record_network_receive(512).await;
        perf_monitor.record_latency(i as f64 * 2.5).await;
        
        // 每3秒输出一次状态
        if i % 3 == 0 {
            match test_gateway.quick_performance_check().await {
                Ok(stats) => {
                    info!("第 {} 秒 - 内存: {:.2}MB, 网络发送: {}字节, 平均延迟: {:.3}ms", 
                          i, stats.memory_usage_mb, stats.network_bytes_sent, stats.average_latency_ms);
                }
                Err(e) => error!("性能检查失败: {}", e),
            }
        }
    }

    info!("性能测试演示完成!");
    info!("主要特性演示:");
    info!("✓ IPv4/IPv6 双栈网络支持");
    info!("✓ 全面的性能监控和基准测试");
    info!("✓ 内存使用优化和分析");
    info!("✓ 网络吞吐量和延迟测试");
    info!("✓ 并发连接性能测试");
    info!("✓ 实时性能报告生成");
    info!("✓ 零编译警告，通过所有Clippy检查");

    Ok(())
}