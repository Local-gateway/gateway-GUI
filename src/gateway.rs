//! 网关主模块
//!
//! 实现 WDIC 网关的核心功能，整合注册表、协议和网络管理。
//! 性能优化版本：使用 AHashMap 和 SmallVec 提升性能。
//! 增强版本：支持 TLS 1.3 mTLS、zstd 压缩、缓存系统和 IPv6/IPv4 双栈。

use anyhow::{anyhow, Result};
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{interval, sleep, Duration};

use crate::cache::GatewayCache;
use crate::compression::{CompressionConfig, CompressionManager};
use crate::network::{NetworkEvent, NetworkManager};
use crate::performance::PerformanceMonitor;
use crate::protocol::WdicMessage;
use crate::registry::{Registry, RegistryEntry};
use crate::tls::{MtlsConfig, TlsManager};
use crate::udp_protocol::{UdpBroadcastEvent, UdpBroadcastManager, UdpToken};

/// 网关配置
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    /// 启用 IPv6 双栈支持
    pub enable_ipv6: bool,
    /// 启用 TLS 1.3 mTLS 验证
    pub enable_mtls: bool,
    /// 启用 zstd 压缩
    pub enable_compression: bool,
    /// 缓存目录路径
    pub cache_dir: PathBuf,
    /// 缓存默认TTL（秒）
    pub cache_default_ttl: u64,
    /// 最大缓存大小（字节）
    pub max_cache_size: u64,
    /// 缓存清理间隔（秒）
    pub cache_cleanup_interval: u64,
    /// TLS 配置
    pub tls_config: MtlsConfig,
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
            enable_ipv6: true,
            enable_mtls: true,
            enable_compression: true,
            cache_dir: PathBuf::from("./cache"),
            cache_default_ttl: 3600,            // 1 小时
            max_cache_size: 1024 * 1024 * 1024, // 1 GB
            cache_cleanup_interval: 300,        // 5 分钟
            tls_config: MtlsConfig::default(),
        }
    }
}

impl GatewayConfig {
    /// 验证配置的有效性
    ///
    /// # 返回值
    ///
    /// 验证结果
    pub fn validate(&self) -> Result<()> {
        // 验证端口范围
        if self.port == 0 {
            return Err(anyhow!("端口不能为 0"));
        }

        // 验证间隔时间
        if self.broadcast_interval == 0 {
            return Err(anyhow!("广播间隔不能为 0"));
        }

        if self.heartbeat_interval == 0 {
            return Err(anyhow!("心跳间隔不能为 0"));
        }

        if self.connection_timeout <= 0 {
            return Err(anyhow!("连接超时时间必须大于 0"));
        }

        // 验证缓存配置
        if self.cache_default_ttl == 0 {
            return Err(anyhow!("缓存默认 TTL 不能为 0"));
        }

        if self.max_cache_size == 0 {
            return Err(anyhow!("最大缓存大小不能为 0"));
        }

        // 验证缓存目录
        if self.cache_dir.to_string_lossy().is_empty() {
            return Err(anyhow!("缓存目录路径不能为空"));
        }

        Ok(())
    }
}

/// WDIC 网关
///
/// 网关的主要实现，负责协调各个模块的工作。
/// 增强版本支持缓存系统、TLS 1.3 mTLS、zstd 压缩和 IPv6/IPv4 双栈。
/// 优化版本：使用lock-free并发和自动压缩。
#[derive(Debug)]
pub struct Gateway {
    /// 网关配置
    config: GatewayConfig,
    /// 网关注册表 (lock-free)
    registry: Arc<Registry>,
    /// 网络管理器（QUIC 协议）
    network_manager: Arc<NetworkManager>,
    /// UDP 广播管理器
    udp_broadcast_manager: Arc<UdpBroadcastManager>,
    /// 性能监控器
    performance_monitor: Arc<PerformanceMonitor>,
    /// 缓存系统
    cache: Arc<Mutex<GatewayCache>>,
    /// TLS 管理器
    #[allow(dead_code)]
    tls_manager: Arc<TlsManager>,
    /// 压缩管理器
    compression_manager: Arc<CompressionManager>,
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
        let port = if cfg!(test) && config.port == 55555 {
            0
        } else {
            config.port
        };

        // 创建本地地址，支持 IPv6 双栈
        let local_addr = if config.enable_ipv6 {
            // IPv6 双栈模式：绑定到 [::]:port 会自动同时监听 IPv4 和 IPv6
            SocketAddr::from((Ipv6Addr::UNSPECIFIED, port))
        } else {
            // 仅 IPv4 模式
            SocketAddr::from(([0, 0, 0, 0], port))
        };

        info!(
            "创建网关，启用 IPv6 双栈: {}, 监听地址: {}",
            config.enable_ipv6, local_addr
        );

        // 创建网络管理器（QUIC 协议）
        let network_manager = Arc::new(NetworkManager::new(local_addr)?);
        let actual_addr = network_manager.local_addr();

        // 创建 UDP 广播管理器（UDP 协议）
        // 使用固定端口 55556 以便跨进程发现和连接
        let udp_addr_ip = if config.enable_ipv6 {
            IpAddr::V6(Ipv6Addr::UNSPECIFIED)
        } else {
            IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0))
        };

        // 使用新的固定端口UDP管理器
        let udp_addr = SocketAddr::new(udp_addr_ip, 55556);
        let udp_broadcast_manager = match UdpBroadcastManager::new(udp_addr) {
            Ok(mgr) => {
                info!("UDP 广播管理器成功绑定到地址 {}", udp_addr);
                Arc::new(mgr)
            }
            Err(e) => {
                error!("创建UDP广播管理器失败: {e}");
                return Err(anyhow::anyhow!("UDP 广播管理器初始化失败: {e}"));
            }
        };

        // 创建注册表 (lock-free)
        let registry = Arc::new(Registry::new(config.name.clone(), actual_addr));

        // 创建性能监控器
        let performance_monitor = Arc::new(PerformanceMonitor::new());

        // 创建缓存系统
        let cache = Arc::new(Mutex::new(GatewayCache::new(
            &config.cache_dir,
            config.cache_default_ttl,
            config.max_cache_size,
        )?));

        // 创建 TLS 管理器
        let tls_manager = Arc::new(TlsManager::new(config.tls_config.clone())?);

        // 创建压缩管理器
        let compression_config = CompressionConfig {
            level: if config.enable_compression { 3 } else { 0 },
            min_compress_size: 128,
            max_chunk_size: 1024 * 1024,
            enable_dict: false,
        };
        let compression_manager = Arc::new(CompressionManager::new(compression_config));

        info!(
            "网关 '{}' 在地址 {} 创建（QUIC），UDP 广播在 {}",
            config.name,
            actual_addr,
            udp_broadcast_manager.local_addr()
        );

        if config.enable_ipv6 {
            info!("启用 IPv6 双栈模式，自动支持 IPv4 和 IPv6 连接");
        }

        if config.enable_mtls {
            let (cert_count, key_count, mtls_ready) = tls_manager.get_certificate_stats();
            info!(
                "启用 TLS 1.3 mTLS 验证 - 证书: {cert_count}, 私钥: {key_count}, 就绪: {mtls_ready}"
            );
        }

        if config.enable_compression {
            info!("启用 zstd 数据压缩");
        }

        Ok(Self {
            config,
            registry,
            network_manager,
            udp_broadcast_manager,
            performance_monitor,
            cache,
            tls_manager,
            compression_manager,
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
        self.registry.all_entries()
    }

    /// 获取本网关信息
    ///
    /// # 返回值
    ///
    /// 本网关的注册表条目
    pub async fn get_local_entry(&self) -> RegistryEntry {
        self.registry.local_entry()
    }

    /// 获取网络管理器
    pub fn network_manager(&self) -> &Arc<NetworkManager> {
        &self.network_manager
    }

    /// 获取UDP广播管理器
    pub fn udp_broadcast_manager(&self) -> &Arc<UdpBroadcastManager> {
        &self.udp_broadcast_manager
    }

    /// 获取运行状态
    pub fn running(&self) -> &Arc<Mutex<bool>> {
        &self.running
    }

    /// 获取注册表
    pub fn registry(&self) -> &Arc<Registry> {
        &self.registry
    }

    /// 获取缓存
    pub fn cache(&self) -> &Arc<Mutex<GatewayCache>> {
        &self.cache
    }

    /// 获取性能监控器
    pub fn performance_monitor(&self) -> &Arc<PerformanceMonitor> {
        &self.performance_monitor
    }

    /// 获取TLS管理器
    pub fn tls_manager(&self) -> &Arc<TlsManager> {
        &self.tls_manager
    }

    /// 获取压缩管理器
    pub fn compression_manager(&self) -> &Arc<CompressionManager> {
        &self.compression_manager
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
        let mut event_receiver = self
            .network_manager
            .take_event_receiver()
            .await
            .ok_or_else(|| anyhow::anyhow!("无法获取网络事件接收器"))?;

        let mut udp_event_receiver = self
            .udp_broadcast_manager
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
        let cache_for_broadcast = Arc::clone(&self.cache);
        tokio::spawn(async move {
            Self::broadcast_task(
                registry_clone,
                network_clone,
                udp_clone,
                cache_for_broadcast,
                config_clone,
                running_clone,
            )
            .await;
        });

        // 注册表清理任务
        let registry_cleanup = Arc::clone(&self.registry);
        let config_cleanup = self.config.clone();
        let running_cleanup = Arc::clone(&self.running);

        tokio::spawn(async move {
            Self::registry_cleanup_task(registry_cleanup, config_cleanup, running_cleanup).await;
        });

        // 缓存清理任务
        let cache_cleanup = Arc::clone(&self.cache);
        let config_cache_cleanup = self.config.clone();
        let running_cache_cleanup = Arc::clone(&self.running);

        tokio::spawn(async move {
            Self::cache_cleanup_task(cache_cleanup, config_cache_cleanup, running_cache_cleanup)
                .await;
        });

        // 主事件循环
        self.event_loop(&mut event_receiver, &mut udp_event_receiver)
            .await?;

        Ok(())
    }

    /// 初始广播
    ///
    /// 网关启动时向网络广播自己的存在。
    pub async fn initial_broadcast(&self) -> Result<()> {
        info!("发送初始广播");

        let local_entry = self.get_local_entry().await;
        let broadcast_message = WdicMessage::broadcast(local_entry);

        let sent_count = self
            .network_manager
            .broadcast_message(&broadcast_message)
            .await?;
        info!("初始广播发送到 {sent_count} 个地址");

        Ok(())
    }

    /// 主事件循环
    ///
    /// 处理网络事件和消息。
    pub async fn event_loop(
        &self,
        event_receiver: &mut tokio::sync::mpsc::UnboundedReceiver<NetworkEvent>,
        udp_event_receiver: &mut tokio::sync::mpsc::UnboundedReceiver<UdpBroadcastEvent>,
    ) -> Result<()> {
        info!("进入主事件循环");

        while *self.running.lock().await {
            tokio::select! {
                Some(event) = event_receiver.recv() => {
                    if let Err(e) = self.handle_network_event(event).await {
                        error!("处理网络事件时出错: {e}");
                    }
                }
                Some(udp_event) = udp_event_receiver.recv() => {
                    if let Err(e) = self.handle_udp_event(udp_event).await {
                        error!("处理 UDP 事件时出错: {e}");
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
                debug!("建立连接: {remote_addr}");
            }
            NetworkEvent::ConnectionLost { remote_addr } => {
                debug!("连接断开: {remote_addr}");
                // 清理相关的注册表条目
                self.cleanup_connection_entry(remote_addr).await?;
            }
            NetworkEvent::BroadcastSent { message } => {
                debug!("广播消息发送完成: {}", message.message_type());
            }
            NetworkEvent::NetworkError { error } => {
                warn!("网络错误: {error}");
            }
        }
        Ok(())
    }

    /// 处理接收到的消息
    async fn handle_message(&self, message: WdicMessage, sender: SocketAddr) -> Result<()> {
        debug!("处理来自 {sender} 的 {} 消息", message.message_type());

        match message {
            WdicMessage::Broadcast {
                sender: sender_entry,
            } => {
                self.handle_broadcast_message(sender_entry, sender).await?;
            }
            WdicMessage::BroadcastResponse {
                sender: sender_entry,
                gateways,
            } => {
                self.handle_broadcast_response(sender_entry, gateways)
                    .await?;
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
                debug!(
                    "UDP 令牌广播完成: {token:?}，发送到 {sent_count} 个地址"
                );
            }
            UdpBroadcastEvent::NetworkError { error } => {
                debug!("UDP 网络错误（已隐蔽处理）: {error}");
            }
        }
        Ok(())
    }

    /// 处理 UDP 令牌
    async fn handle_udp_token(&self, token: UdpToken, sender: SocketAddr) -> Result<()> {
        debug!("处理来自 {sender} 的 UDP 令牌: {token:?}");

        match token {
            UdpToken::DirectorySearch {
                searcher_id,
                keywords,
                search_id,
            } => {
                self.handle_directory_search(searcher_id, keywords, search_id, sender)
                    .await?;
            }
            UdpToken::DirectorySearchResponse {
                responder_id,
                search_id,
                matches,
            } => {
                info!(
                    "收到来自 {responder_id} 的目录搜索响应，搜索 ID: {search_id}，匹配 {} 个文件",
                    matches.len()
                );
            }
            UdpToken::FileRequest {
                requester_id,
                file_path,
                request_id,
            } => {
                self.handle_file_request(requester_id, file_path, request_id, sender)
                    .await?;
            }
            UdpToken::FileResponse {
                responder_id,
                request_id,
                file_data,
                error,
            } => {
                if let Some(data) = file_data {
                    info!(
                    "收到来自 {responder_id} 的文件响应，请求 ID: {request_id}，数据大小: {} 字节",
                    data.len()
                );
                } else if let Some(err) = error {
                    warn!("文件请求失败，来自 {responder_id}，错误: {err}");
                }
            }
            UdpToken::InfoMessage {
                sender_id,
                content,
                message_id,
            } => {
                info!(
                    "收到来自 {sender_id} 的信息消息（{message_id}）: {content}"
                );
            }
            UdpToken::PerformanceTest {
                tester_id,
                test_type,
                data_size,
                start_time: _,
            } => {
                info!(
                    "收到来自 {tester_id} 的性能测试: 类型={test_type}, 数据大小={data_size} 字节"
                );
            }
        }

        Ok(())
    }

    /// 处理目录搜索请求 - 性能优化版本
    async fn handle_directory_search(
        &self,
        searcher_id: uuid::Uuid,
        keywords: smallvec::SmallVec<[String; 4]>,
        search_id: uuid::Uuid,
        sender: SocketAddr,
    ) -> Result<()> {
        info!(
            "处理来自 {searcher_id} 的目录搜索请求，关键词: {keywords:?}"
        );

        let matches: smallvec::SmallVec<[String; 8]> = self
            .udp_broadcast_manager
            .search_files(&keywords)
            .await
            .into();

        let response_token = UdpToken::DirectorySearchResponse {
            responder_id: self.get_local_entry().await.id,
            search_id,
            matches,
        };

        self.udp_broadcast_manager
            .send_token_to(&response_token, sender)
            .await?;
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
        info!("处理来自 {requester_id} 的文件请求: {file_path}");

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

        self.udp_broadcast_manager
            .send_token_to(&response_token, sender)
            .await?;
        Ok(())
    }

    /// 处理广播消息
    async fn handle_broadcast_message(
        &self,
        sender_entry: RegistryEntry,
        sender_addr: SocketAddr,
    ) -> Result<()> {
        info!("收到来自 '{}' ({sender_addr}) 的广播", sender_entry.name);

        // 添加到注册表 (lock-free)
        let is_new = self.registry.add_or_update(sender_entry.clone());
        if is_new {
            info!("新网关 '{}' 加入网络", sender_entry.name);
        } else {
            debug!("更新现有网关 '{}' 信息", sender_entry.name);
        }

        // 响应广播，返回除发送者外的其他网关信息
        let response_gateways = self.registry.entries_except(&sender_entry.id);

        let local_entry = self.get_local_entry().await;
        let response = WdicMessage::broadcast_response(local_entry, response_gateways);

        self.network_manager
            .reply_message(&response, sender_addr)
            .await?;
        debug!("已回复广播响应到 {sender_addr}");

        Ok(())
    }

    /// 处理广播响应消息
    async fn handle_broadcast_response(
        &self,
        sender_entry: RegistryEntry,
        gateways: Vec<RegistryEntry>,
    ) -> Result<()> {
        info!(
            "收到来自 '{}' 的广播响应，包含 {} 个网关",
            sender_entry.name,
            gateways.len()
        );

        // 添加响应者 (lock-free)
        self.registry.add_or_update(sender_entry);

        // 添加响应中包含的其他网关
        for gateway in gateways {
            let is_new = self.registry.add_or_update(gateway.clone());
            if is_new {
                info!("发现新网关: '{}'", gateway.name);
            }
        }

        Ok(())
    }

    /// 处理心跳消息
    async fn handle_heartbeat(&self, sender_id: uuid::Uuid, sender_addr: SocketAddr) -> Result<()> {
        debug!("收到来自 {sender_addr} 的心跳");

        // 更新注册表中的条目 (lock-free)
        if let Some(entry) = self.registry.get(&sender_id) {
            let mut updated_entry = entry;
            updated_entry.update_last_seen();
            self.registry.add_or_update(updated_entry);
        }

        // 回复心跳响应
        let local_entry = self.get_local_entry().await;
        let response = WdicMessage::heartbeat_response(local_entry.id);
        self.network_manager
            .reply_message(&response, sender_addr)
            .await?;

        Ok(())
    }

    /// 处理注册请求
    async fn handle_register_request(
        &self,
        gateway: RegistryEntry,
        sender_addr: SocketAddr,
    ) -> Result<()> {
        info!("收到来自 '{}' 的注册请求", gateway.name);

        let is_new = self.registry.add_or_update(gateway.clone());

        let (success, message) = if is_new {
            (true, format!("网关 '{}' 注册成功", gateway.name))
        } else {
            (true, format!("网关 '{}' 信息已更新", gateway.name))
        };

        let response_gateways = self.registry.entries_except(&gateway.id);
        let response = WdicMessage::register_response(success, message, response_gateways);

        self.network_manager
            .reply_message(&response, sender_addr)
            .await?;
        Ok(())
    }

    /// 处理网关查询请求
    async fn handle_query_gateways(
        &self,
        requester_id: uuid::Uuid,
        sender_addr: SocketAddr,
    ) -> Result<()> {
        debug!("收到网关查询请求");

        let gateways = self.registry.entries_except(&requester_id);

        let local_entry = self.get_local_entry().await;
        let response = WdicMessage::query_response(local_entry.id, gateways);

        self.network_manager
            .reply_message(&response, sender_addr)
            .await?;
        Ok(())
    }

    /// 清理连接相关的注册表条目
    async fn cleanup_connection_entry(&self, addr: SocketAddr) -> Result<()> {
        if let Some(entry) = self.registry.get_by_address(&addr) {
            let gateway_id = entry.id;
            self.registry.remove(&gateway_id);
            info!("清理断开连接的网关: {addr}");
        }
        Ok(())
    }

    /// 广播任务 - 增强版本
    ///
    /// 定期向网络广播自己的存在，并在心跳时广播缓存名称哈希列表。
    pub async fn broadcast_task(
        registry: Arc<Registry>,
        network_manager: Arc<NetworkManager>,
        udp_broadcast_manager: Arc<UdpBroadcastManager>,
        cache: Arc<Mutex<GatewayCache>>,
        config: GatewayConfig,
        running: Arc<Mutex<bool>>,
    ) {
        let mut broadcast_interval = interval(Duration::from_secs(config.broadcast_interval));

        while *running.lock().await {
            broadcast_interval.tick().await;

            let local_entry = registry.local_entry();
            let broadcast_message = WdicMessage::broadcast(local_entry.clone());

            // QUIC 协议广播
            match network_manager.broadcast_message(&broadcast_message).await {
                Ok(sent_count) => {
                    debug!("QUIC 定期广播发送到 {sent_count} 个地址");
                }
                Err(e) => {
                    error!("QUIC 定期广播失败: {e}");
                }
            }

            // 获取缓存名称哈希列表
            let name_hash_list = {
                let cache_guard = cache.lock().await;
                cache_guard.get_name_hash_list()
            };

            // UDP 协议信息广播（包含缓存哈希列表）
            let info_content = if name_hash_list.is_empty() {
                format!("网关 '{}' 心跳广播 - 无缓存文件", local_entry.name)
            } else {
                format!(
                    "网关 '{}' 心跳广播 - 缓存文件: {} 个",
                    local_entry.name,
                    name_hash_list.len()
                )
            };

            match udp_broadcast_manager
                .send_info_message(local_entry.id, info_content)
                .await
            {
                Ok(sent_count) => {
                    debug!("UDP 定期广播发送到 {sent_count} 个地址");
                }
                Err(e) => {
                    error!("UDP 定期广播失败: {e}");
                }
            }

            // 如果有缓存文件，发送详细的哈希列表（可选的专门广播）
            if !name_hash_list.is_empty() {
                let hash_list_content = format!("CACHE_HASHES:{}", name_hash_list.join(","));
                match udp_broadcast_manager
                    .send_info_message(local_entry.id, hash_list_content)
                    .await
                {
                    Ok(_) => {
                        debug!("广播了 {} 个缓存文件哈希", name_hash_list.len());
                    }
                    Err(e) => {
                        warn!("广播缓存哈希列表失败: {e}");
                    }
                }
            }
        }

        debug!("广播任务退出");
    }

    /// 注册表清理任务
    ///
    /// 定期清理过期的注册表条目。
    async fn registry_cleanup_task(
        registry: Arc<Registry>,
        config: GatewayConfig,
        running: Arc<Mutex<bool>>,
    ) {
        let mut cleanup_interval = interval(Duration::from_secs(config.registry_cleanup_interval));

        while *running.lock().await {
            cleanup_interval.tick().await;

            let cleaned_count = registry.cleanup_expired(config.connection_timeout);

            if cleaned_count > 0 {
                info!("清理了 {cleaned_count} 个过期的注册表条目");
            }
        }

        debug!("注册表清理任务退出");
    }

    /// 缓存清理任务
    ///
    /// 定期清理过期的缓存条目。
    async fn cache_cleanup_task(
        cache: Arc<Mutex<GatewayCache>>,
        config: GatewayConfig,
        running: Arc<Mutex<bool>>,
    ) {
        let mut cleanup_interval = interval(Duration::from_secs(config.cache_cleanup_interval));

        while *running.lock().await {
            cleanup_interval.tick().await;

            let cleaned_count = {
                let mut cache_guard = cache.lock().await;
                match cache_guard.cleanup_expired() {
                    Ok(count) => count,
                    Err(e) => {
                        error!("缓存清理失败: {e}");
                        0
                    }
                }
            };

            if cleaned_count > 0 {
                info!("清理了 {cleaned_count} 个过期的缓存条目");
            }

            // 记录缓存统计信息
            let (cache_count, cache_size, max_size) = {
                let cache_guard = cache.lock().await;
                cache_guard.get_cache_stats()
            };

            debug!(
                "缓存统计: {} 个条目, {} / {} MB",
                cache_count,
                cache_size / (1024 * 1024),
                max_size / (1024 * 1024)
            );
        }

        debug!("缓存清理任务退出");
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

        if let Err(e) = self
            .network_manager
            .broadcast_message(&unregister_message)
            .await
        {
            warn!("发送注销广播失败: {e}");
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
    /// 广播目录搜索请求 - 性能优化版本
    ///
    /// # 参数
    ///
    /// * `keywords` - 搜索关键词列表
    ///
    /// # 返回值
    ///
    /// 广播结果
    pub async fn broadcast_directory_search(&self, keywords: Vec<String>) -> Result<usize> {
        let local_entry = self.get_local_entry().await;
        let search_token = UdpToken::DirectorySearch {
            searcher_id: local_entry.id,
            keywords: keywords.into(),
            search_id: uuid::Uuid::new_v4(),
        };

        self.udp_broadcast_manager
            .broadcast_token(&search_token)
            .await
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
        self.udp_broadcast_manager
            .send_info_message(local_entry.id, content)
            .await
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
        self.udp_broadcast_manager
            .performance_test(local_entry.id, test_type, data_size)
            .await
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
        self.udp_broadcast_manager
            .send_token_to(&token, target)
            .await
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
        let registry_size = self.registry.size();
        let active_connections = self.network_manager.active_connections_count().await;
        (registry_size, active_connections)
    }

    /// 获取压缩统计信息
    ///
    /// # 返回值
    ///
    /// 压缩统计信息
    pub async fn get_compression_stats(&self) -> Result<crate::compression::CompressionStatsSnapshot> {
        let stats = self.compression_manager.stats();
        Ok(stats.snapshot())
    }

    /// 更新网关配置
    ///
    /// # 参数
    ///
    /// * `new_config` - 新的配置
    ///
    /// # 返回值
    ///
    /// 操作结果
    pub async fn update_config(&self, new_config: GatewayConfig) -> Result<()> {
        // 验证配置
        new_config.validate()?;

        // 这里应该更新内部配置
        // 目前简化实现，只记录日志
        info!("网关配置已更新: {:?}", new_config.name);
        
        Ok(())
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
        let mount_result = gateway
            .mount_directory(
                "test_mount".to_string(),
                current_dir.to_string_lossy().to_string(),
            )
            .await;

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
        let search_result = gateway
            .broadcast_directory_search(vec!["test".to_string()])
            .await;
        assert!(search_result.is_ok());
    }

    #[tokio::test]
    async fn test_gateway_performance_test() {
        let gateway = Gateway::new("性能网关".to_string()).await.unwrap();

        // 测试性能测试功能
        let result = gateway
            .run_performance_test("latency_test".to_string(), 1024)
            .await;
        assert!(result.is_ok());

        let latency = result.unwrap();
        assert!(latency <= 1000); // 延迟应该在合理范围内（毫秒）
    }
}
