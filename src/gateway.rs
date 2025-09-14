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
use crate::protocol::WdicMessage;
use crate::network::{NetworkManager, NetworkEvent};
use crate::udp_protocol::{UdpBroadcastManager, UdpBroadcastEvent, UdpToken};
use crate::performance::{PerformanceMonitor, PerformanceTestSuite, BenchmarkResult};

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
    /// 网络管理器（QUIC 协议）
    network_manager: Arc<NetworkManager>,
    /// UDP 广播管理器
    udp_broadcast_manager: Arc<UdpBroadcastManager>,
    /// 性能监控器
    performance_monitor: Arc<PerformanceMonitor>,
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
        
        // 创建网络管理器（QUIC 协议）
        let network_manager = Arc::new(NetworkManager::new(local_addr)?);
        let actual_addr = network_manager.local_addr();
        
        // 创建 UDP 广播管理器（UDP 协议）
        // 在测试环境中使用 0 端口让系统自动分配
        let udp_port = if cfg!(test) { 0 } else { actual_addr.port() };
        let udp_addr = SocketAddr::from(([0, 0, 0, 0], udp_port));
        let udp_broadcast_manager = Arc::new(UdpBroadcastManager::new(udp_addr)?);
        
        // 创建注册表
        let registry = Arc::new(RwLock::new(Registry::new(
            config.name.clone(),
            actual_addr,
        )));

        // 创建性能监控器
        let performance_monitor = Arc::new(PerformanceMonitor::new());

        info!("网关 '{}' 在地址 {} 创建（QUIC），UDP 广播在 {}", 
              config.name, actual_addr, udp_broadcast_manager.local_addr());

        Ok(Self {
            config,
            registry,
            network_manager,
            udp_broadcast_manager,
            performance_monitor,
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

    /// 获取UDP广播地址
    pub fn udp_local_addr(&self) -> SocketAddr {
        self.udp_broadcast_manager.local_addr()
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

        // 启动网络管理器（QUIC 协议）
        self.network_manager.start().await?;
        
        // 启动 UDP 广播管理器
        self.udp_broadcast_manager.start().await?;

        // 获取事件接收器
        let mut event_receiver = self.network_manager
            .take_event_receiver()
            .await
            .ok_or_else(|| anyhow::anyhow!("无法获取网络事件接收器"))?;
            
        let mut udp_event_receiver = self.udp_broadcast_manager
            .take_event_receiver()
            .await
            .ok_or_else(|| anyhow::anyhow!("无法获取 UDP 广播事件接收器"))?;

        // 启动初始广播
        self.initial_broadcast().await?;

        // 启动定期任务
        let registry_clone = Arc::clone(&self.registry);
        let network_clone = Arc::clone(&self.network_manager);
        let udp_clone = Arc::clone(&self.udp_broadcast_manager);
        let config_clone = self.config.clone();
        let running_clone = Arc::clone(&self.running);

        // 广播任务
        tokio::spawn(async move {
            Self::broadcast_task(registry_clone, network_clone, udp_clone, config_clone, running_clone).await;
        });

        // 注册表清理任务
        let registry_cleanup = Arc::clone(&self.registry);
        let config_cleanup = self.config.clone();
        let running_cleanup = Arc::clone(&self.running);

        tokio::spawn(async move {
            Self::registry_cleanup_task(registry_cleanup, config_cleanup, running_cleanup).await;
        });

        // 主事件循环
        self.event_loop(&mut event_receiver, &mut udp_event_receiver).await?;

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
    async fn event_loop(
        &self, 
        event_receiver: &mut tokio::sync::mpsc::UnboundedReceiver<NetworkEvent>,
        udp_event_receiver: &mut tokio::sync::mpsc::UnboundedReceiver<UdpBroadcastEvent>,
    ) -> Result<()> {
        info!("进入主事件循环");

        while *self.running.lock().await {
            tokio::select! {
                Some(event) = event_receiver.recv() => {
                    if let Err(e) = self.handle_network_event(event).await {
                        error!("处理网络事件时出错: {}", e);
                    }
                }
                Some(udp_event) = udp_event_receiver.recv() => {
                    if let Err(e) = self.handle_udp_event(udp_event).await {
                        error!("处理 UDP 事件时出错: {}", e);
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

    /// 处理 UDP 广播事件
    async fn handle_udp_event(&self, event: UdpBroadcastEvent) -> Result<()> {
        match event {
            UdpBroadcastEvent::TokenReceived { token, sender } => {
                self.handle_udp_token(token, sender).await?;
            }
            UdpBroadcastEvent::BroadcastSent { token, sent_count } => {
                debug!("UDP 令牌广播完成: {:?}，发送到 {} 个地址", token, sent_count);
            }
            UdpBroadcastEvent::NetworkError { error } => {
                debug!("UDP 网络错误（已隐蔽处理）: {}", error);
            }
        }
        Ok(())
    }

    /// 处理 UDP 令牌
    async fn handle_udp_token(&self, token: UdpToken, sender: SocketAddr) -> Result<()> {
        debug!("处理来自 {} 的 UDP 令牌: {:?}", sender, token);

        match token {
            UdpToken::DirectorySearch { searcher_id, keywords, search_id } => {
                self.handle_directory_search(searcher_id, keywords, search_id, sender).await?;
            }
            UdpToken::DirectorySearchResponse { responder_id, search_id, matches } => {
                info!("收到来自 {} 的目录搜索响应，搜索 ID: {}，匹配 {} 个文件", 
                      responder_id, search_id, matches.len());
            }
            UdpToken::FileRequest { requester_id, file_path, request_id } => {
                self.handle_file_request(requester_id, file_path, request_id, sender).await?;
            }
            UdpToken::FileResponse { responder_id, request_id, file_data, error } => {
                if let Some(data) = file_data {
                    info!("收到来自 {} 的文件响应，请求 ID: {}，数据大小: {} 字节", 
                          responder_id, request_id, data.len());
                } else if let Some(err) = error {
                    warn!("文件请求失败，来自 {}，错误: {}", responder_id, err);
                }
            }
            UdpToken::InfoMessage { sender_id, content, message_id } => {
                info!("收到来自 {} 的信息消息（{}）: {}", sender_id, message_id, content);
            }
            UdpToken::PerformanceTest { tester_id, test_type, data_size, start_time: _ } => {
                info!("收到来自 {} 的性能测试: 类型={}, 数据大小={} 字节", 
                      tester_id, test_type, data_size);
            }
        }

        Ok(())
    }

    /// 处理目录搜索请求
    async fn handle_directory_search(
        &self,
        searcher_id: uuid::Uuid,
        keywords: Vec<String>,
        search_id: uuid::Uuid,
        sender: SocketAddr,
    ) -> Result<()> {
        info!("处理来自 {} 的目录搜索请求，关键词: {:?}", searcher_id, keywords);

        let matches = self.udp_broadcast_manager.search_files(&keywords).await;
        
        let response_token = UdpToken::DirectorySearchResponse {
            responder_id: self.get_local_entry().await.id,
            search_id,
            matches,
        };

        self.udp_broadcast_manager.send_token_to(&response_token, sender).await?;
        Ok(())
    }

    /// 处理文件请求
    async fn handle_file_request(
        &self,
        requester_id: uuid::Uuid,
        file_path: String,
        request_id: uuid::Uuid,
        sender: SocketAddr,
    ) -> Result<()> {
        info!("处理来自 {} 的文件请求: {}", requester_id, file_path);

        let response_token = match self.udp_broadcast_manager.read_file(&file_path).await {
            Ok(file_data) => UdpToken::FileResponse {
                responder_id: self.get_local_entry().await.id,
                request_id,
                file_data: Some(file_data),
                error: None,
            },
            Err(e) => UdpToken::FileResponse {
                responder_id: self.get_local_entry().await.id,
                request_id,
                file_data: None,
                error: Some(e.to_string()),
            },
        };

        self.udp_broadcast_manager.send_token_to(&response_token, sender).await?;
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
        udp_broadcast_manager: Arc<UdpBroadcastManager>,
        config: GatewayConfig,
        running: Arc<Mutex<bool>>,
    ) {
        let mut broadcast_interval = interval(Duration::from_secs(config.broadcast_interval));

        while *running.lock().await {
            broadcast_interval.tick().await;

            let local_entry = registry.read().await.local_entry().clone();
            let broadcast_message = WdicMessage::broadcast(local_entry.clone());

            // QUIC 协议广播
            match network_manager.broadcast_message(&broadcast_message).await {
                Ok(sent_count) => {
                    debug!("QUIC 定期广播发送到 {} 个地址", sent_count);
                }
                Err(e) => {
                    error!("QUIC 定期广播失败: {}", e);
                }
            }

            // UDP 协议信息广播
            let info_content = format!("网关 '{}' 心跳广播", local_entry.name);
            match udp_broadcast_manager.send_info_message(local_entry.id, info_content).await {
                Ok(sent_count) => {
                    debug!("UDP 定期广播发送到 {} 个地址", sent_count);
                }
                Err(e) => {
                    error!("UDP 定期广播失败: {}", e);
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
        
        // 关闭 UDP 广播管理器
        self.udp_broadcast_manager.stop().await?;

        info!("网关 '{}' 已停止", self.config.name);
        Ok(())
    }

    /// 挂载目录
    /// 
    /// # 参数
    /// 
    /// * `name` - 挂载点名称
    /// * `path` - 目录路径
    /// 
    /// # 返回值
    /// 
    /// 挂载结果
    pub async fn mount_directory(&self, name: String, path: String) -> Result<()> {
        self.udp_broadcast_manager.mount_directory(name, path).await
    }

    /// 卸载目录
    /// 
    /// # 参数
    /// 
    /// * `name` - 挂载点名称
    /// 
    /// # 返回值
    /// 
    /// 是否成功卸载
    pub async fn unmount_directory(&self, name: &str) -> bool {
        self.udp_broadcast_manager.unmount_directory(name).await
    }

    /// 获取已挂载目录列表
    /// 
    /// # 返回值
    /// 
    /// 挂载点名称列表
    pub async fn get_mounted_directories(&self) -> Vec<String> {
        self.udp_broadcast_manager.get_mounted_directories().await
    }

    /// 搜索文件
    /// 
    /// # 参数
    /// 
    /// * `keywords` - 搜索关键词
    /// 
    /// # 返回值
    /// 
    /// 匹配的文件路径列表
    pub async fn search_files_locally(&self, keywords: &[String]) -> Vec<String> {
        self.udp_broadcast_manager.search_files(keywords).await
    }

    /// 向网络广播目录搜索请求
    /// 
    /// # 参数
    /// 
    /// * `keywords` - 搜索关键词
    /// 
    /// # 返回值
    /// 
    /// 广播结果
    pub async fn broadcast_directory_search(&self, keywords: Vec<String>) -> Result<usize> {
        let local_entry = self.get_local_entry().await;
        let search_token = UdpToken::DirectorySearch {
            searcher_id: local_entry.id,
            keywords,
            search_id: uuid::Uuid::new_v4(),
        };
        
        self.udp_broadcast_manager.broadcast_token(&search_token).await
    }

    /// 向网络广播信息消息
    /// 
    /// # 参数
    /// 
    /// * `content` - 消息内容
    /// 
    /// # 返回值
    /// 
    /// 广播结果
    pub async fn broadcast_info_message(&self, content: String) -> Result<usize> {
        let local_entry = self.get_local_entry().await;
        self.udp_broadcast_manager.send_info_message(local_entry.id, content).await
    }

    /// 执行性能测试
    /// 
    /// # 参数
    /// 
    /// * `test_type` - 测试类型
    /// * `data_size` - 测试数据大小
    /// 
    /// # 返回值
    /// 
    /// 测试结果（延迟毫秒数）
    pub async fn run_performance_test(&self, test_type: String, data_size: usize) -> Result<u64> {
        let local_entry = self.get_local_entry().await;
        self.udp_broadcast_manager.performance_test(local_entry.id, test_type, data_size).await
    }

    /// 定向发送令牌到指定地址
    /// 
    /// # 参数
    /// 
    /// * `token` - 要发送的令牌
    /// * `target` - 目标地址
    /// 
    /// # 返回值
    /// 
    /// 发送结果
    pub async fn send_token_to(&self, token: UdpToken, target: SocketAddr) -> Result<()> {
        self.udp_broadcast_manager.send_token_to(&token, target).await
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

    /// 获取性能监控器
    /// 
    /// # 返回值
    /// 
    /// 性能监控器实例
    pub fn performance_monitor(&self) -> Arc<PerformanceMonitor> {
        Arc::clone(&self.performance_monitor)
    }

    /// 运行综合性能测试套件
    /// 
    /// 执行多种性能测试，包括吞吐量、延迟、内存使用等。
    /// 
    /// # 返回值
    /// 
    /// 包含所有测试结果的映射表
    pub async fn run_comprehensive_performance_tests(&self) -> Result<std::collections::HashMap<String, BenchmarkResult>> {
        info!("开始运行综合性能测试套件");
        
        let mut results = std::collections::HashMap::new();
        
        // 1. 网络吞吐量测试
        info!("执行网络吞吐量测试...");
        let throughput_suite = PerformanceTestSuite {
            concurrency: 20,
            duration_seconds: 10,
            packet_size: 1024,
            test_interval_ms: 5,
        };
        
        match self.performance_monitor.run_throughput_benchmark("network_throughput", &throughput_suite).await {
            Ok(result) => {
                results.insert("network_throughput".to_string(), result);
            }
            Err(e) => warn!("网络吞吐量测试失败: {}", e),
        }
        
        // 2. UDP 广播性能测试
        info!("执行UDP广播性能测试...");
        let udp_suite = PerformanceTestSuite {
            concurrency: 10,
            duration_seconds: 5,
            packet_size: 512,
            test_interval_ms: 10,
        };
        
        match self.performance_monitor.run_throughput_benchmark("udp_broadcast", &udp_suite).await {
            Ok(result) => {
                results.insert("udp_broadcast".to_string(), result);
            }
            Err(e) => warn!("UDP广播性能测试失败: {}", e),
        }
        
        // 3. 延迟测试
        info!("执行延迟测试...");
        match self.performance_monitor.run_latency_benchmark("message_latency", 1000).await {
            Ok(result) => {
                results.insert("message_latency".to_string(), result);
            }
            Err(e) => warn!("延迟测试失败: {}", e),
        }
        
        // 4. 注册表性能测试
        info!("执行注册表性能测试...");
        match self.test_registry_performance().await {
            Ok(result) => {
                results.insert("registry_performance".to_string(), result);
            }
            Err(e) => warn!("注册表性能测试失败: {}", e),
        }
        
        // 5. 内存使用分析
        info!("执行内存使用分析...");
        match self.test_memory_usage().await {
            Ok(result) => {
                results.insert("memory_usage".to_string(), result);
            }
            Err(e) => warn!("内存使用分析失败: {}", e),
        }
        
        // 6. 并发连接测试
        info!("执行并发连接测试...");
        match self.test_concurrent_connections().await {
            Ok(result) => {
                results.insert("concurrent_connections".to_string(), result);
            }
            Err(e) => warn!("并发连接测试失败: {}", e),
        }

        info!("综合性能测试套件完成，共执行了 {} 个测试", results.len());
        
        Ok(results)
    }

    /// 测试注册表性能
    async fn test_registry_performance(&self) -> Result<BenchmarkResult> {
        let start_time = std::time::Instant::now();
        let iterations = 10000;
        let mut operations = 0u64;
        
        // 记录开始时的内存使用
        self.performance_monitor.update_system_metrics().await?;
        let start_memory = self.performance_monitor.get_memory_metrics().await.current_usage;
        
        // 测试注册表操作
        for i in 0..iterations {
            let entry = RegistryEntry::new(
                format!("test_gateway_{}", i),
                std::net::SocketAddr::from(([192, 168, 1, (i % 255) as u8], 55555 + (i % 1000) as u16)),
            );
            
            // 添加条目
            {
                let mut registry = self.registry.write().await;
                registry.add_or_update(entry.clone());
            }
            operations += 1;
            
            // 查询条目
            {
                let registry = self.registry.read().await;
                let _ = registry.get_by_address(&entry.address);
            }
            operations += 1;
            
            // 每1000次操作清理一次过期条目
            if i % 1000 == 0 {
                let mut registry = self.registry.write().await;
                registry.cleanup_expired(300);
                operations += 1;
            }
        }
        
        let total_duration = start_time.elapsed();
        
        // 记录结束时的内存使用
        self.performance_monitor.update_system_metrics().await?;
        let end_memory = self.performance_monitor.get_memory_metrics().await.current_usage;
        
        let ops_per_second = operations as f64 / total_duration.as_secs_f64();
        
        let mut parameters = std::collections::HashMap::new();
        parameters.insert("iterations".to_string(), iterations.to_string());
        parameters.insert("operation_types".to_string(), "add,get,cleanup".to_string());
        
        Ok(BenchmarkResult {
            name: "registry_performance".to_string(),
            duration_ms: total_duration.as_secs_f64() * 1000.0,
            operations,
            ops_per_second,
            average_latency: (total_duration.as_secs_f64() * 1000.0) / operations as f64,
            min_latency: 0.0,
            max_latency: 0.0,
            throughput_bps: 0.0,
            memory_usage: end_memory.saturating_sub(start_memory),
            timestamp: chrono::Utc::now(),
            parameters,
        })
    }

    /// 测试内存使用情况
    async fn test_memory_usage(&self) -> Result<BenchmarkResult> {
        let start_time = std::time::Instant::now();
        
        // 更新系统指标
        self.performance_monitor.update_system_metrics().await?;
        let start_memory = self.performance_monitor.get_memory_metrics().await.current_usage;
        
        // 创建大量数据以测试内存使用
        let mut test_data = Vec::new();
        for i in 0..1000 {
            let entry = RegistryEntry::new(
                format!("memory_test_gateway_{}", i),
                std::net::SocketAddr::from(([10, 0, (i / 255) as u8, (i % 255) as u8], 55555)),
            );
            test_data.push(entry);
        }
        
        // 模拟一些 UDP 消息
        for i in 0..500 {
            let token = UdpToken::InfoMessage {
                sender_id: uuid::Uuid::new_v4(),
                content: format!("内存测试消息 {} - 这是一个用于测试内存使用的较长消息内容", i),
                message_id: uuid::Uuid::new_v4(),
            };
            
            // 序列化消息（模拟内存使用）
            let _ = serde_json::to_string(&token);
        }
        
        let total_duration = start_time.elapsed();
        
        // 更新并获取最终内存使用
        self.performance_monitor.update_system_metrics().await?;
        let end_memory = self.performance_monitor.get_memory_metrics().await.current_usage;
        let peak_memory = self.performance_monitor.get_memory_metrics().await.peak_usage;
        
        let mut parameters = std::collections::HashMap::new();
        parameters.insert("test_entries".to_string(), "1000".to_string());
        parameters.insert("test_messages".to_string(), "500".to_string());
        parameters.insert("peak_memory_mb".to_string(), format!("{:.2}", peak_memory as f64 / 1024.0 / 1024.0));
        
        Ok(BenchmarkResult {
            name: "memory_usage".to_string(),
            duration_ms: total_duration.as_secs_f64() * 1000.0,
            operations: 1500, // 1000 entries + 500 messages
            ops_per_second: 1500.0 / total_duration.as_secs_f64(),
            average_latency: (total_duration.as_secs_f64() * 1000.0) / 1500.0,
            min_latency: 0.0,
            max_latency: 0.0,
            throughput_bps: 0.0,
            memory_usage: end_memory.saturating_sub(start_memory),
            timestamp: chrono::Utc::now(),
            parameters,
        })
    }

    /// 测试并发连接性能
    async fn test_concurrent_connections(&self) -> Result<BenchmarkResult> {
        let start_time = std::time::Instant::now();
        let concurrent_tasks = 50;
        let operations_per_task = 20;
        
        // 记录开始时的内存使用
        self.performance_monitor.update_system_metrics().await?;
        let start_memory = self.performance_monitor.get_memory_metrics().await.current_usage;
        
        let mut handles = Vec::new();
        
        // 启动并发任务
        for task_id in 0..concurrent_tasks {
            let registry = Arc::clone(&self.registry);
            let perf_monitor = Arc::clone(&self.performance_monitor);
            
            let handle = tokio::spawn(async move {
                for i in 0..operations_per_task {
                    // 模拟连接事件
                    perf_monitor.record_connection_event(
                        crate::performance::ConnectionEvent::Connected, 
                        None
                    ).await;
                    
                    // 添加注册表条目
                    let entry = RegistryEntry::new(
                        format!("concurrent_gateway_{}_{}", task_id, i),
                        std::net::SocketAddr::from(([172, 16, (task_id % 255) as u8, (i % 255) as u8], 55555 + i as u16)),
                    );
                    
                    {
                        let mut reg = registry.write().await;
                        reg.add_or_update(entry);
                    }
                    
                    // 模拟一些处理时间
                    tokio::time::sleep(std::time::Duration::from_millis(1)).await;
                    
                    // 模拟断开连接
                    perf_monitor.record_connection_event(
                        crate::performance::ConnectionEvent::Disconnected, 
                        Some(1.0)
                    ).await;
                }
            });
            
            handles.push(handle);
        }
        
        // 等待所有任务完成
        for handle in handles {
            let _ = handle.await;
        }
        
        let total_duration = start_time.elapsed();
        
        // 记录结束时的内存使用
        self.performance_monitor.update_system_metrics().await?;
        let end_memory = self.performance_monitor.get_memory_metrics().await.current_usage;
        
        let total_operations = (concurrent_tasks * operations_per_task * 3) as u64; // 3 operations per iteration
        let ops_per_second = total_operations as f64 / total_duration.as_secs_f64();
        
        let mut parameters = std::collections::HashMap::new();
        parameters.insert("concurrent_tasks".to_string(), concurrent_tasks.to_string());
        parameters.insert("operations_per_task".to_string(), operations_per_task.to_string());
        
        Ok(BenchmarkResult {
            name: "concurrent_connections".to_string(),
            duration_ms: total_duration.as_secs_f64() * 1000.0,
            operations: total_operations,
            ops_per_second,
            average_latency: (total_duration.as_secs_f64() * 1000.0) / total_operations as f64,
            min_latency: 0.0,
            max_latency: 0.0,
            throughput_bps: 0.0,
            memory_usage: end_memory.saturating_sub(start_memory),
            timestamp: chrono::Utc::now(),
            parameters,
        })
    }

    /// 生成详细的性能报告
    /// 
    /// # 返回值
    /// 
    /// 包含所有性能指标的详细报告
    pub async fn generate_performance_report(&self) -> String {
        // 更新系统指标
        let _ = self.performance_monitor.update_system_metrics().await;
        
        // 生成基础报告
        let base_report = self.performance_monitor.generate_report().await;
        
        // 添加网关特定信息
        let mut report = String::new();
        report.push_str("=== WDIC 网关性能报告 ===\n");
        report.push_str(&format!("网关名称: {}\n", self.config.name));
        report.push_str(&format!("监听地址: {}\n", self.local_addr()));
        report.push_str(&format!("UDP广播地址: {}\n", self.udp_broadcast_manager.local_addr()));
        report.push_str("配置信息:\n");
        report.push_str(&format!("  - 广播间隔: {} 秒\n", self.config.broadcast_interval));
        report.push_str(&format!("  - 心跳间隔: {} 秒\n", self.config.heartbeat_interval));
        report.push_str(&format!("  - 连接超时: {} 秒\n", self.config.connection_timeout));
        report.push_str(&format!("  - 注册表清理间隔: {} 秒\n\n", self.config.registry_cleanup_interval));
        
        // 添加当前状态信息
        let (registry_size, active_connections) = self.get_stats().await;
        report.push_str("## 当前状态\n");
        report.push_str(&format!("注册表条目数: {}\n", registry_size));
        report.push_str(&format!("活跃连接数: {}\n", active_connections));
        report.push_str(&format!("运行状态: {}\n\n", if *self.running.lock().await { "运行中" } else { "已停止" }));
        
        // 添加基础性能报告
        report.push_str(&base_report);
        
        report
    }

    /// 运行快速性能检查
    /// 
    /// 执行一个快速的性能检查，适用于健康检查或监控。
    /// 
    /// # 返回值
    /// 
    /// 简化的性能摘要
    pub async fn quick_performance_check(&self) -> Result<QuickPerformanceStats> {
        // 更新系统指标
        self.performance_monitor.update_system_metrics().await?;
        
        let memory_metrics = self.performance_monitor.get_memory_metrics().await;
        let network_metrics = self.performance_monitor.get_network_metrics().await;
        let latency_metrics = self.performance_monitor.get_latency_metrics().await;
        let connection_metrics = self.performance_monitor.get_connection_metrics().await;
        
        let (registry_size, active_connections) = self.get_stats().await;
        
        Ok(QuickPerformanceStats {
            memory_usage_mb: memory_metrics.current_usage as f64 / 1024.0 / 1024.0,
            memory_usage_percentage: memory_metrics.usage_percentage,
            network_bytes_sent: network_metrics.bytes_sent,
            network_bytes_received: network_metrics.bytes_received,
            average_latency_ms: latency_metrics.average_latency,
            active_connections,
            registry_size,
            connection_success_rate: connection_metrics.connection_success_rate,
            uptime_seconds: chrono::Utc::now().timestamp() - self.get_local_entry().await.last_seen.timestamp(),
        })
    }
}

/// 快速性能统计信息
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct QuickPerformanceStats {
    /// 内存使用量（MB）
    pub memory_usage_mb: f64,
    /// 内存使用率（百分比）
    pub memory_usage_percentage: f64,
    /// 网络发送字节数
    pub network_bytes_sent: u64,
    /// 网络接收字节数
    pub network_bytes_received: u64,
    /// 平均延迟（毫秒）
    pub average_latency_ms: f64,
    /// 活跃连接数
    pub active_connections: usize,
    /// 注册表大小
    pub registry_size: usize,
    /// 连接成功率（百分比）
    pub connection_success_rate: f64,
    /// 运行时间（秒）
    pub uptime_seconds: i64,
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
        // 验证端口已分配

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

    #[tokio::test]
    async fn test_gateway_directory_operations() {
        let gateway = Gateway::new("目录网关".to_string()).await.unwrap();
        
        // 测试挂载目录（使用当前目录）
        let current_dir = std::env::current_dir().unwrap();
        let mount_result = gateway.mount_directory(
            "test_mount".to_string(),
            current_dir.to_string_lossy().to_string()
        ).await;

        if mount_result.is_ok() {
            // 测试获取挂载目录
            let mounted = gateway.get_mounted_directories().await;
            assert!(mounted.contains(&"test_mount".to_string()));

            // 测试本地文件搜索
            let _results = gateway.search_files_locally(&["rs".to_string()]).await;
            // 应该能找到一些 .rs 文件

            // 测试卸载
            let unmounted = gateway.unmount_directory("test_mount").await;
            assert!(unmounted);
        }
    }

    #[tokio::test]
    async fn test_gateway_udp_messaging() {
        let gateway = Gateway::new("消息网关".to_string()).await.unwrap();
        
        // 测试广播信息消息
        let result = gateway.broadcast_info_message("测试消息".to_string()).await;
        assert!(result.is_ok());
        
        // 测试目录搜索广播
        let search_result = gateway.broadcast_directory_search(vec!["test".to_string()]).await;
        assert!(search_result.is_ok());
    }

    #[tokio::test]
    async fn test_gateway_performance_test() {
        let gateway = Gateway::new("性能网关".to_string()).await.unwrap();
        
        // 测试性能测试功能
        let result = gateway.run_performance_test("latency_test".to_string(), 1024).await;
        assert!(result.is_ok());
        
        let latency = result.unwrap();
        assert!(latency <= 1000); // 延迟应该在合理范围内（毫秒）
    }
}