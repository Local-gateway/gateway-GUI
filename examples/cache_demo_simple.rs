//! 简化的缓存系统演示
//!
//! 此演示展示 WDIC 网关缓存系统的基本功能

use log::info;
use std::sync::Arc;
use wdic_gateway::{Gateway, GatewayConfig};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    info!("=== WDIC 网关缓存系统演示 ===");

    // 创建网关配置
    let config = GatewayConfig::default();
    
    // 创建网关实例
    let gateway = Arc::new(Gateway::new("cache-demo".to_string()).await?);
    
    info!("网关初始化完成");

    // 获取缓存统计信息
    let stats = {
        let cache = gateway.cache();
        let cache_guard = cache.lock().await;
        cache_guard.get_stats().await
    };
    
    match stats {
        Ok((cache_count, cache_size)) => {
            info!("当前缓存状态:");
            info!("  - 缓存文件数: {}", cache_count);
            info!("  - 缓存总大小: {} 字节", cache_size);
        }
        Err(e) => {
            info!("获取缓存统计失败: {}", e);
        }
    }

    // 显示网关基本统计
    let (registry_size, active_connections) = gateway.get_stats().await;
    info!("网关统计:");
    info!("  - 注册表条目: {}", registry_size);
    info!("  - 活跃连接: {}", active_connections);

    info!("缓存演示完成！");
    Ok(())
}
