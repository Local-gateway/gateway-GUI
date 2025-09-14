//! WDIC 网关应用程序入口
//!
//! 基于 QUIC 的本地网关实现，提供 P2P 网络发现和注册表管理功能。

use wdic_gateway::Gateway;
use log::{info, error};
use tokio::signal;
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    // 初始化日志记录
    env_logger::init();

    info!("启动 WDIC 网关应用程序");

    // 创建网关实例
    let gateway = Gateway::new("本地网关".to_string()).await?;
    
    info!("网关创建成功，监听地址: {}", gateway.local_addr());

    // 设置信号处理
    let gateway_clone = std::sync::Arc::new(gateway);
    let gateway_for_signal = gateway_clone.clone();

    tokio::spawn(async move {
        match signal::ctrl_c().await {
            Ok(()) => {
                info!("收到中断信号，正在关闭网关...");
                if let Err(e) = gateway_for_signal.stop().await {
                    error!("关闭网关时出错: {}", e);
                }
            }
            Err(err) => {
                error!("监听中断信号时出错: {}", err);
            }
        }
    });

    // 运行网关
    match gateway_clone.run().await {
        Ok(()) => {
            info!("网关正常退出");
        }
        Err(e) => {
            error!("网关运行时出错: {}", e);
            return Err(e);
        }
    }

    Ok(())
}
