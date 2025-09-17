//! WDIC 网关缓存系统演示
//!
//! 展示网关缓存系统的功能，包括文件缓存、压缩、哈希计算和网络广播。

use log::{info, warn, error};
use std::sync::Arc;
use tokio::time::{sleep, Duration};
use wdic_gateway::{Gateway, GatewayConfig};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 初始化日志记录
    env_logger::init();

    info!("启动 WDIC 网关缓存系统演示");

    // 创建自定义配置，启用所有增强功能
    let config = GatewayConfig {
        name: "缓存演示网关".to_string(),
        port: 0, // 使用 0 端口让系统自动分配
        enable_ipv6: true,
        enable_mtls: true,
        enable_compression: true,
        cache_default_ttl: 60,            // 1分钟TTL用于演示
        max_cache_size: 10 * 1024 * 1024, // 10MB缓存
        cache_cleanup_interval: 30,       // 30秒清理一次
        ..Default::default()
    };

    // 创建网关实例
    let gateway = Gateway::with_config(config).await?;
    let gateway_clone = Arc::new(gateway);

    info!("网关创建成功");

    // 缓存演示结束，停止网关
    match gateway_clone.stop().await {
        Ok(_) => info!("网关正常停止"),
        Err(e) => error!("网关停止时出现错误: {e}"),
    }

    info!("缓存系统演示完成!");
    Ok(())
}
