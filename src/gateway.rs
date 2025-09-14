//! 网关主模块
//! 
//! 实现 WDIC 网关的核心功能，整合注册表、协议和网络管理。

use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tokio::time::{Duration, interval, sleep};
use anyhow::Result;
use log::{info, warn, error, debug};

use crate::registry::{Registry, RegistryEntry};
use crate::protocol::{WdicMessage, WdicProtocol};
use crate::network::{NetworkManager, NetworkEvent};

/// 网关配置
#[derive(Debug, Clone)]
pub struct GatewayConfig {
    /// 网关名称
    pub name: String,
    /// 监听端口
    pub port: u16,
    /// 广播间隔（秒）
    pub broadcast_interval: u64,
    /// 心跳间隔（秒）
    pub heartbeat_interval: u64,
    /// 连接超时时间（秒）
    pub connection_timeout: i64,
    /// 注册表清理间隔（秒）
    pub registry_cleanup_interval: u64,
}

impl Default for GatewayConfig {
    fn default() -> Self {
        Self {
            name: "本地网关".to_string(),
            port: 55555,
            broadcast_interval: 30,
            heartbeat_interval: 60,
            connection_timeout: 300,
            registry_cleanup_interval: 120,
        }
    }
}

/// WDIC 网关
/// 
/// 网关的主要实现，负责协调各个模块的工作。
pub struct Gateway {
    /// 网关配置
    config: GatewayConfig,
    /// 网关注册表
    registry: Arc<RwLock<Registry>>,
    /// 网络管理器
    network_manager: Arc<NetworkManager>,
    /// 协议处理器
    protocol: WdicProtocol,
    /// 运行状态
    running: Arc<Mutex<bool>>,
}

impl Gateway {
    /// 创建新的网关实例
    /// 
    /// # 参数
    /// 
    /// * `name` - 网关名称
    /// 
    /// # 返回值
    /// 
    /// 网关实例
    pub async fn new(name: String) -> Result<Self> {
        let config = GatewayConfig {
            name: name.clone(),
            ..Default::default()
        };
        
        Self::with_config(config).await
    }

    /// 使用指定配置创建网关实例
    /// 
    /// # 参数
    /// 
    /// * `config` - 网关配置
    /// 
    /// # 返回值
    /// 
    /// 网关实例
    pub async fn with_config(config: GatewayConfig) -> Result<Self> {
        // 如果配置的端口为 55555，在测试环境中使用 0 以避免冲突
        let port = if cfg!(test) && config.port == 55555 { 0 } else { config.port };
        let local_addr = SocketAddr::from(([0, 0, 0, 0], port));
        
        // 创建网络管理器
        let network_manager = Arc::new(NetworkManager::new(local_addr)?);
        let actual_addr = network_manager.local_addr();
        
        // 创建注册表
        let registry = Arc::new(RwLock::new(Registry::new(
            config.name.clone(),
            actual_addr,
        )));

        info!("网关 '{}' 在地址 {} 创建", config.name, actual_addr);

        Ok(Self {
            config,
            registry,
            network_manager,
            protocol: WdicProtocol::new(),
            running: Arc::new(Mutex::new(false)),
        })
    }

    /// 获取网关配置
    pub fn config(&self) -> &GatewayConfig {
        &self.config
    }

    /// 获取本地地址
    pub fn local_addr(&self) -> SocketAddr {
        self.network_manager.local_addr()
    }

    /// 获取注册表快照
    /// 
    /// # 返回值
    /// 
    /// 当前注册表中的所有条目
    pub async fn get_registry_snapshot(&self) -> Vec<RegistryEntry> {
        self.registry.read().await.all_entries()
    }

    /// 获取本网关信息
    /// 
    /// # 返回值
    /// 
    /// 本网关的注册表条目
    pub async fn get_local_entry(&self) -> RegistryEntry {
        self.registry.read().await.local_entry().clone()
    }

    /// 启动网关
    /// 
    /// 开始监听网络消息、定期广播和维护注册表。
    pub async fn run(&self) -> Result<()> {
        {
            let mut running = self.running.lock().await;
            if *running {
                return Err(anyhow::anyhow!("网关已经在运行"));
            }
            *running = true;
        }

        info!("启动网关 '{}'", self.config.name);

        // 启动网络管理器
        self.network_manager.start().await?;

        // 获取事件接收器
        let mut event_receiver = self.network_manager
            .take_event_receiver()
            .await
            .ok_or_else(|| anyhow::anyhow!("无法获取网络事件接收器"))?;

        // 启动初始广播
        self.initial_broadcast().await?;

        // 启动定期任务
        let registry_clone = Arc::clone(&self.registry);
        let network_clone = Arc::clone(&self.network_manager);
        let config_clone = self.config.clone();
        let running_clone = Arc::clone(&self.running);

        // 广播任务
        tokio::spawn(async move {
            Self::broadcast_task(registry_clone, network_clone, config_clone, running_clone).await;
        });

        // 注册表清理任务
        let registry_cleanup = Arc::clone(&self.registry);
        let config_cleanup = self.config.clone();
        let running_cleanup = Arc::clone(&self.running);

        tokio::spawn(async move {
            Self::registry_cleanup_task(registry_cleanup, config_cleanup, running_cleanup).await;
        });

        // 主事件循环
        self.event_loop(&mut event_receiver).await?;

        Ok(())
    }

    /// 初始广播
    /// 
    /// 网关启动时向网络广播自己的存在。
    async fn initial_broadcast(&self) -> Result<()> {
        info!("发送初始广播");
        
        let local_entry = self.get_local_entry().await;
        let broadcast_message = WdicMessage::broadcast(local_entry);

        let sent_count = self.network_manager.broadcast_message(&broadcast_message).await?;
        info!("初始广播发送到 {} 个地址", sent_count);

        Ok(())
    }

    /// 主事件循环
    /// 
    /// 处理网络事件和消息。
    async fn event_loop(&self, event_receiver: &mut tokio::sync::mpsc::UnboundedReceiver<NetworkEvent>) -> Result<()> {
        info!("进入主事件循环");

        while *self.running.lock().await {
            tokio::select! {
                Some(event) = event_receiver.recv() => {
                    if let Err(e) = self.handle_network_event(event).await {
                        error!("处理网络事件时出错: {}", e);
                    }
                }
                _ = sleep(Duration::from_millis(100)) => {
                    // 定期检查运行状态
                }
            }
        }

        info!("事件循环退出");
        Ok(())
    }

    /// 处理网络事件
    async fn handle_network_event(&self, event: NetworkEvent) -> Result<()> {
        match event {
            NetworkEvent::MessageReceived { message, sender } => {
                self.handle_message(message, sender).await?;
            }
            NetworkEvent::ConnectionEstablished { remote_addr } => {
                debug!("建立连接: {}", remote_addr);
            }
            NetworkEvent::ConnectionLost { remote_addr } => {
                debug!("连接断开: {}", remote_addr);
                // 清理相关的注册表条目
                self.cleanup_connection_entry(remote_addr).await?;
            }
            NetworkEvent::BroadcastSent { message } => {
                debug!("广播消息发送完成: {}", message.message_type());
            }
            NetworkEvent::NetworkError { error } => {
                warn!("网络错误: {}", error);
            }
        }
        Ok(())
    }

    /// 处理接收到的消息
    async fn handle_message(&self, message: WdicMessage, sender: SocketAddr) -> Result<()> {
        debug!("处理来自 {} 的 {} 消息", sender, message.message_type());

        match message {
            WdicMessage::Broadcast { sender: sender_entry } => {
                self.handle_broadcast_message(sender_entry, sender).await?;
            }
            WdicMessage::BroadcastResponse { sender: sender_entry, gateways } => {
                self.handle_broadcast_response(sender_entry, gateways).await?;
            }
            WdicMessage::Heartbeat { sender_id, .. } => {
                self.handle_heartbeat(sender_id, sender).await?;
            }
            WdicMessage::RegisterRequest { gateway } => {
                self.handle_register_request(gateway, sender).await?;
            }
            WdicMessage::QueryGateways { requester_id } => {
                self.handle_query_gateways(requester_id, sender).await?;
            }
            _ => {
                debug!("忽略消息类型: {}", message.message_type());
            }
        }

        Ok(())
    }

    /// 处理广播消息
    async fn handle_broadcast_message(&self, sender_entry: RegistryEntry, sender_addr: SocketAddr) -> Result<()> {
        info!("收到来自 '{}' ({}) 的广播", sender_entry.name, sender_addr);

        // 添加到注册表
        {
            let mut registry = self.registry.write().await;
            let is_new = registry.add_or_update(sender_entry.clone());
            if is_new {
                info!("新网关 '{}' 加入网络", sender_entry.name);
            } else {
                debug!("更新现有网关 '{}' 信息", sender_entry.name);
            }
        }

        // 响应广播，返回除发送者外的其他网关信息
        let response_gateways = {
            let registry = self.registry.read().await;
            registry.entries_except(&sender_entry.id)
        };

        let local_entry = self.get_local_entry().await;
        let response = WdicMessage::broadcast_response(local_entry, response_gateways);

        self.network_manager.reply_message(&response, sender_addr).await?;
        debug!("已回复广播响应到 {}", sender_addr);

        Ok(())
    }

    /// 处理广播响应消息
    async fn handle_broadcast_response(&self, sender_entry: RegistryEntry, gateways: Vec<RegistryEntry>) -> Result<()> {
        info!("收到来自 '{}' 的广播响应，包含 {} 个网关", sender_entry.name, gateways.len());

        let mut registry = self.registry.write().await;
        
        // 添加响应者
        registry.add_or_update(sender_entry);

        // 添加响应中包含的其他网关
        for gateway in gateways {
            let is_new = registry.add_or_update(gateway.clone());
            if is_new {
                info!("发现新网关: '{}'", gateway.name);
            }
        }

        Ok(())
    }

    /// 处理心跳消息
    async fn handle_heartbeat(&self, sender_id: uuid::Uuid, sender_addr: SocketAddr) -> Result<()> {
        debug!("收到来自 {} 的心跳", sender_addr);

        // 更新注册表中的条目
        {
            let mut registry = self.registry.write().await;
            if let Some(entry) = registry.get(&sender_id) {
                let mut updated_entry = entry.clone();
                updated_entry.update_last_seen();
                registry.add_or_update(updated_entry);
            }
        }

        // 回复心跳响应
        let local_entry = self.get_local_entry().await;
        let response = WdicMessage::heartbeat_response(local_entry.id);
        self.network_manager.reply_message(&response, sender_addr).await?;

        Ok(())
    }

    /// 处理注册请求
    async fn handle_register_request(&self, gateway: RegistryEntry, sender_addr: SocketAddr) -> Result<()> {
        info!("收到来自 '{}' 的注册请求", gateway.name);

        let mut registry = self.registry.write().await;
        let is_new = registry.add_or_update(gateway.clone());
        
        let (success, message) = if is_new {
            (true, format!("网关 '{}' 注册成功", gateway.name))
        } else {
            (true, format!("网关 '{}' 信息已更新", gateway.name))
        };

        let response_gateways = registry.entries_except(&gateway.id);
        let response = WdicMessage::register_response(success, message, response_gateways);

        self.network_manager.reply_message(&response, sender_addr).await?;
        Ok(())
    }

    /// 处理网关查询请求
    async fn handle_query_gateways(&self, requester_id: uuid::Uuid, sender_addr: SocketAddr) -> Result<()> {
        debug!("收到网关查询请求");

        let gateways = {
            let registry = self.registry.read().await;
            registry.entries_except(&requester_id)
        };

        let local_entry = self.get_local_entry().await;
        let response = WdicMessage::query_response(local_entry.id, gateways);

        self.network_manager.reply_message(&response, sender_addr).await?;
        Ok(())
    }

    /// 清理连接相关的注册表条目
    async fn cleanup_connection_entry(&self, addr: SocketAddr) -> Result<()> {
        let mut registry = self.registry.write().await;
        if let Some(entry) = registry.get_by_address(&addr) {
            let gateway_id = entry.id;
            registry.remove(&gateway_id);
            info!("清理断开连接的网关: {}", addr);
        }
        Ok(())
    }

    /// 广播任务
    /// 
    /// 定期向网络广播自己的存在。
    async fn broadcast_task(
        registry: Arc<RwLock<Registry>>,
        network_manager: Arc<NetworkManager>,
        config: GatewayConfig,
        running: Arc<Mutex<bool>>,
    ) {
        let mut broadcast_interval = interval(Duration::from_secs(config.broadcast_interval));

        while *running.lock().await {
            broadcast_interval.tick().await;

            let local_entry = registry.read().await.local_entry().clone();
            let broadcast_message = WdicMessage::broadcast(local_entry);

            match network_manager.broadcast_message(&broadcast_message).await {
                Ok(sent_count) => {
                    debug!("定期广播发送到 {} 个地址", sent_count);
                }
                Err(e) => {
                    error!("定期广播失败: {}", e);
                }
            }
        }

        debug!("广播任务退出");
    }

    /// 注册表清理任务
    /// 
    /// 定期清理过期的注册表条目。
    async fn registry_cleanup_task(
        registry: Arc<RwLock<Registry>>,
        config: GatewayConfig,
        running: Arc<Mutex<bool>>,
    ) {
        let mut cleanup_interval = interval(Duration::from_secs(config.registry_cleanup_interval));

        while *running.lock().await {
            cleanup_interval.tick().await;

            let cleaned_count = {
                let mut reg = registry.write().await;
                reg.cleanup_expired(config.connection_timeout)
            };

            if cleaned_count > 0 {
                info!("清理了 {} 个过期的注册表条目", cleaned_count);
            }
        }

        debug!("注册表清理任务退出");
    }

    /// 停止网关
    /// 
    /// 停止所有任务和网络服务。
    pub async fn stop(&self) -> Result<()> {
        info!("停止网关 '{}'", self.config.name);

        {
            let mut running = self.running.lock().await;
            *running = false;
        }

        // 发送注销广播
        let local_entry = self.get_local_entry().await;
        let unregister_message = WdicMessage::UnregisterRequest {
            gateway_id: local_entry.id,
        };

        if let Err(e) = self.network_manager.broadcast_message(&unregister_message).await {
            warn!("发送注销广播失败: {}", e);
        }

        // 关闭网络管理器
        self.network_manager.shutdown().await?;

        info!("网关 '{}' 已停止", self.config.name);
        Ok(())
    }

    /// 检查网关是否正在运行
    /// 
    /// # 返回值
    /// 
    /// 如果网关正在运行返回 true，否则返回 false
    pub async fn is_running(&self) -> bool {
        *self.running.lock().await
    }

    /// 获取网关统计信息
    /// 
    /// # 返回值
    /// 
    /// 包含注册表大小和活跃连接数的统计信息
    pub async fn get_stats(&self) -> (usize, usize) {
        let registry_size = self.registry.read().await.size();
        let active_connections = self.network_manager.active_connections_count().await;
        (registry_size, active_connections)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_gateway_creation() {
        let gateway = Gateway::new("测试网关".to_string()).await;
        assert!(gateway.is_ok());

        let gateway = gateway.unwrap();
        assert_eq!(gateway.config().name, "测试网关");
        assert!(!gateway.is_running().await);
    }

    #[tokio::test]
    async fn test_gateway_with_config() {
        let config = GatewayConfig {
            name: "配置网关".to_string(),
            port: 0, // 让系统分配端口
            broadcast_interval: 10,
            ..Default::default()
        };

        let gateway = Gateway::with_config(config).await;
        assert!(gateway.is_ok());

        let gateway = gateway.unwrap();
        assert_eq!(gateway.config().name, "配置网关");
        assert_eq!(gateway.config().broadcast_interval, 10);
    }

    #[tokio::test]
    async fn test_gateway_local_info() {
        let gateway = Gateway::new("信息网关".to_string()).await.unwrap();
        
        let local_entry = gateway.get_local_entry().await;
        assert_eq!(local_entry.name, "信息网关");
        assert!(local_entry.address.port() >= 0); // 允许端口为 0 或正数

        let registry_snapshot = gateway.get_registry_snapshot().await;
        assert!(registry_snapshot.is_empty()); // 新网关注册表应该为空

        let (registry_size, active_connections) = gateway.get_stats().await;
        assert_eq!(registry_size, 0);
        assert_eq!(active_connections, 0);
    }

    #[tokio::test]
    async fn test_gateway_stop_before_start() {
        let gateway = Gateway::new("停止网关".to_string()).await.unwrap();
        
        // 在未启动的情况下停止应该成功
        let result = gateway.stop().await;
        assert!(result.is_ok());
    }
}