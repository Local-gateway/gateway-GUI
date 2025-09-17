//! UDP 广播协议演示示例
//!
//! 这个示例展示了如何使用新的 UDP 广播功能，包括：
//! - 目录挂载和搜索
//! - 文件发送
//! - 信息广播
//! - 性能测试

use log::info;
use std::net::SocketAddr;
use tokio::time::{sleep, Duration};
use uuid::Uuid;
use wdic_gateway::{Gateway, GatewayConfig, UdpToken};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 初始化日志
    env_logger::init();

    info!("启动 UDP 广播协议演示");

    // 创建网关实例，使用端口 0 进行自动分配
    let config = GatewayConfig {
        name: "UDP演示网关".to_string(),
        port: 0, // 使用 0 端口让系统自动分配
        ..Default::default()
    };
    let gateway = Gateway::with_config(config).await?;

    info!("网关创建成功");
    info!("QUIC 地址: {}", gateway.local_addr());

    // 演示目录挂载功能
    info!("=== 目录挂载演示 ===");
    let current_dir = std::env::current_dir()?;
    match gateway
        .mount_directory(
            "src_code".to_string(),
            current_dir.join("src").to_string_lossy().to_string(),
        )
        .await
    {
        Ok(()) => {
            info!("成功挂载源代码目录");

            // 获取挂载的目录列表
            let mounted = gateway.get_mounted_directories().await;
            info!("当前挂载的目录: {mounted:?}");

            // 本地文件搜索演示
            let search_results = gateway.search_files_locally(&["rs".to_string()]).await;
            info!("本地搜索 'rs' 文件结果: {} 个文件", search_results.len());
            for file in search_results.iter().take(3) {
                info!("  - {file}");
            }
        }
        Err(e) => {
            info!("目录挂载失败（正常，用于演示）: {e}");
        }
    }

    // 演示信息广播功能
    info!("=== 信息广播演示 ===");
    let broadcast_result = gateway
        .broadcast_info_message("这是一条来自 UDP 演示网关的测试消息！".to_string())
        .await?;
    info!("信息广播发送到 {broadcast_result} 个地址");

    // 演示目录搜索广播
    info!("=== 目录搜索广播演示 ===");
    let search_broadcast_result = gateway
        .broadcast_directory_search(vec!["rs".to_string(), "toml".to_string()])
        .await?;
    info!("目录搜索广播发送到 {search_broadcast_result} 个地址");

    // 演示性能测试功能
    info!("=== 性能测试演示 ===");
    let latency_1k = gateway
        .run_performance_test("latency_test".to_string(), 1024)
        .await?;
    info!("1KB 数据延迟测试结果: {latency_1k} 毫秒");

    let latency_10k = gateway
        .run_performance_test("throughput_test".to_string(), 10240)
        .await?;
    info!("10KB 数据延迟测试结果: {latency_10k} 毫秒");

    // 演示定向令牌发送（发送到本地地址用于演示）
    info!("=== 定向令牌发送演示 ===");
    let target_addr: SocketAddr = "127.0.0.1:55556".parse()?;
    let custom_token = UdpToken::InfoMessage {
        sender_id: Uuid::new_v4(),
        content: "这是一个定向发送的测试令牌".to_string(),
        message_id: Uuid::new_v4(),
    };

    match gateway.send_token_to(custom_token, target_addr).await {
        Ok(()) => info!("成功发送定向令牌到 {target_addr}"),
        Err(e) => info!("定向令牌发送失败（正常，没有监听者）: {e}"),
    }

    // 演示多种令牌类型的创建
    info!("=== 令牌类型演示 ===");

    // 文件请求令牌
    let file_request = UdpToken::FileRequest {
        requester_id: Uuid::new_v4(),
        file_path: "/path/to/file.txt".to_string(),
        request_id: Uuid::new_v4(),
    };
    info!("创建文件请求令牌: {file_request:?}");

    // 性能测试令牌
    let perf_test = UdpToken::PerformanceTest {
        tester_id: Uuid::new_v4(),
        test_type: "bandwidth_test".to_string(),
        data_size: 4096,
        start_time: chrono::Utc::now(),
    };
    info!("创建性能测试令牌: {perf_test:?}");

    // 等待一段时间以便观察日志
    info!("等待 3 秒以便观察网络活动...");
    sleep(Duration::from_secs(3)).await;

    // 清理演示
    info!("=== 清理演示 ===");
    let unmounted = gateway.unmount_directory("src_code").await;
    if unmounted {
        info!("成功卸载目录");
    }

    // 显示最终统计信息
    let (registry_size, active_connections) = gateway.get_stats().await;
    info!(
        "最终统计 - 注册表大小: {registry_size}, 活跃连接: {active_connections}"
    );

    info!("UDP 广播协议演示完成！");

    // 优雅停止
    gateway.stop().await?;

    Ok(())
}
