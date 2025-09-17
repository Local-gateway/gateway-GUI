//! WDIC 网关使用示例
//!
//! 展示如何创建和运行一个 WDIC 网关实例。

use log::info;
use std::time::Duration;
use tokio::time::sleep;
use wdic_gateway::{Gateway, GatewayConfig};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 初始化日志记录
    env_logger::init();

    info!("启动 WDIC 网关示例");

    // 创建自定义配置
    let config = GatewayConfig {
        name: "示例网关".to_string(),
        port: 55556,                   // 使用不同的端口以避免冲突
        broadcast_interval: 15,        // 15秒广播一次
        heartbeat_interval: 30,        // 30秒心跳一次
        connection_timeout: 180,       // 3分钟连接超时
        registry_cleanup_interval: 60, // 1分钟清理一次注册表
        ..Default::default()           // 使用其他默认配置
    };

    // 创建网关实例
    let gateway = Gateway::with_config(config).await?;

    info!("网关创建成功，监听地址: {}", gateway.local_addr());

    // 在后台运行网关
    let gateway_clone = std::sync::Arc::new(gateway);
    let gateway_for_task = gateway_clone.clone();

    tokio::spawn(async move {
        if let Err(e) = gateway_for_task.run().await {
            eprintln!("网关运行错误: {e}");
        }
    });

    // 等待网关启动
    sleep(Duration::from_secs(2)).await;

    // 定期显示网关状态
    for i in 1..=10 {
        let (registry_size, active_connections) = gateway_clone.get_stats().await;
        info!(
            "第 {i} 次检查 - 注册表大小: {registry_size}, 活跃连接: {active_connections}"
        );

        let local_entry = gateway_clone.get_local_entry().await;
        info!(
            "本地网关信息: {} ({})",
            local_entry.name, local_entry.address
        );

        let registry_snapshot = gateway_clone.get_registry_snapshot().await;
        if !registry_snapshot.is_empty() {
            info!("发现的其他网关:");
            for entry in registry_snapshot {
                info!("  - {} ({})", entry.name, entry.address);
            }
        } else {
            info!("暂未发现其他网关");
        }

        sleep(Duration::from_secs(5)).await;
    }

    // 停止网关
    info!("停止网关");
    gateway_clone.stop().await?;

    info!("示例程序结束");
    Ok(())
}
