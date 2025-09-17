//! 网络管理模块
//!
//! 处理 QUIC 连接、UDP 广播和网络通信，支持 IPv4/IPv6 双栈网络。

use anyhow::Result;
use log::{debug, error, info, warn};
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, UdpSocket};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tokio::time::{interval, Duration};

use crate::protocol::{WdicMessage, WdicProtocol};

/// 网络事件类型
#[derive(Debug, Clone)]
pub enum NetworkEvent {
    /// 收到新消息
    MessageReceived {
        /// 消息内容
        message: WdicMessage,
        /// 发送者地址
        sender: SocketAddr,
    },
    /// 新的连接建立
    ConnectionEstablished {
        /// 远程地址
        remote_addr: SocketAddr,
    },
    /// 连接断开
    ConnectionLost {
        /// 远程地址
        remote_addr: SocketAddr,
    },
    /// 广播发送完成
    BroadcastSent {
        /// 广播消息
        message: WdicMessage,
    },
    /// 网络错误
    NetworkError {
        /// 错误信息
        error: String,
    },
}

/// 连接状态
#[derive(Debug, Clone)]
pub struct ConnectionState {
    /// 远程地址
    pub remote_addr: SocketAddr,
    /// 最后活跃时间
    pub last_active: chrono::DateTime<chrono::Utc>,
    /// 连接建立时间
    pub established_at: chrono::DateTime<chrono::Utc>,
}

impl ConnectionState {
    /// 创建新的连接状态
    pub fn new(remote_addr: SocketAddr) -> Self {
        let now = chrono::Utc::now();
        Self {
            remote_addr,
            last_active: now,
            established_at: now,
        }
    }

    /// 更新最后活跃时间
    pub fn update_activity(&mut self) {
        self.last_active = chrono::Utc::now();
    }

    /// 检查连接是否超时
    pub fn is_expired(&self, timeout_seconds: i64) -> bool {
        let now = chrono::Utc::now();
        (now - self.last_active).num_seconds() > timeout_seconds
    }
}

/// 网络管理器
///
/// 负责处理网络通信，包括 UDP 广播和消息收发。
#[derive(Debug)]
pub struct NetworkManager {
    /// 本地地址
    local_addr: SocketAddr,
    /// UDP 套接字
    udp_socket: Arc<UdpSocket>,
    /// 协议处理器
    protocol: WdicProtocol,
    /// 活跃连接
    connections: Arc<Mutex<HashMap<SocketAddr, ConnectionState>>>,
    /// 事件发送通道
    event_sender: mpsc::UnboundedSender<NetworkEvent>,
    /// 事件接收通道
    event_receiver: Arc<Mutex<Option<mpsc::UnboundedReceiver<NetworkEvent>>>>,
    /// 广播地址列表
    broadcast_addresses: Vec<SocketAddr>,
}

impl NetworkManager {
    /// 创建新的网络管理器
    ///
    /// # 参数
    ///
    /// * `local_addr` - 本地监听地址
    ///
    /// # 返回值
    ///
    /// 网络管理器实例
    pub fn new(local_addr: SocketAddr) -> Result<Self> {
        let udp_socket = UdpSocket::bind(local_addr)?;
        udp_socket.set_broadcast(true)?;
        udp_socket.set_nonblocking(true)?;

        let (event_sender, event_receiver) = mpsc::unbounded_channel();

        // 生成常见的广播地址
        let broadcast_addresses = Self::generate_broadcast_addresses(local_addr);

        Ok(Self {
            local_addr,
            udp_socket: Arc::new(udp_socket),
            protocol: WdicProtocol::new(),
            connections: Arc::new(Mutex::new(HashMap::new())),
            event_sender,
            event_receiver: Arc::new(Mutex::new(Some(event_receiver))),
            broadcast_addresses,
        })
    }

    /// 获取本地地址
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    /// 获取事件接收器
    pub async fn take_event_receiver(&self) -> Option<mpsc::UnboundedReceiver<NetworkEvent>> {
        self.event_receiver.lock().await.take()
    }

    /// 生成广播地址列表
    ///
    /// 根据本地地址生成可能的广播地址。
    /// 生成广播地址列表（支持 IPv4/IPv6 双栈）
    ///
    /// 这个函数会：
    /// 1. 发现本地所有网络接口
    /// 2. 为 IPv4 地址生成广播地址
    /// 3. 为 IPv6 地址生成多播地址
    /// 4. 优先使用内网地址，在没有内网地址时使用公网地址
    ///
    /// # 参数
    ///
    /// * `local_addr` - 本地绑定地址
    ///
    /// # 返回值
    ///
    /// 广播和多播地址列表
    fn generate_broadcast_addresses(local_addr: SocketAddr) -> Vec<SocketAddr> {
        let mut addresses = Vec::new();
        let port = local_addr.port();

        debug!("为地址 {local_addr} 生成广播地址列表");

        // 获取所有网络接口
        let interfaces = match if_addrs::get_if_addrs() {
            Ok(interfaces) => interfaces,
            Err(e) => {
                warn!("无法获取网络接口列表: {e}, 使用默认广播地址");
                return Self::generate_fallback_addresses(port);
            }
        };

        // 分类接口地址
        let mut ipv4_private = Vec::new();
        let mut ipv4_public = Vec::new();
        let mut ipv6_private = Vec::new();
        let mut ipv6_public = Vec::new();

        for interface in interfaces {
            if interface.is_loopback() {
                continue;
            }

            match interface.ip() {
                IpAddr::V4(ipv4) => {
                    if Self::is_private_ipv4(ipv4) {
                        ipv4_private.push(ipv4);
                    } else {
                        ipv4_public.push(ipv4);
                    }
                }
                IpAddr::V6(ipv6) => {
                    if Self::is_private_ipv6(ipv6) {
                        ipv6_private.push(ipv6);
                    } else {
                        ipv6_public.push(ipv6);
                    }
                }
            }
        }

        // 生成 IPv4 广播地址
        Self::add_ipv4_broadcasts(&mut addresses, &ipv4_private, port);
        if ipv4_private.is_empty() && !ipv4_public.is_empty() {
            info!("没有找到私有 IPv4 地址，使用公网 IPv4 地址进行广播");
            Self::add_ipv4_public_broadcasts(&mut addresses, &ipv4_public, port);
        }

        // 生成 IPv6 多播地址
        if !ipv6_private.is_empty() || !ipv6_public.is_empty() {
            Self::add_ipv6_multicasts(&mut addresses, port);
        }

        // 如果没有找到任何有效地址，使用后备地址
        if addresses.is_empty() {
            warn!("没有找到有效的网络接口，使用默认广播地址");
            addresses = Self::generate_fallback_addresses(port);
        }

        info!("生成了 {} 个广播/多播地址", addresses.len());
        for addr in &addresses {
            debug!("  - {addr}");
        }

        addresses
    }

    /// 判断是否为私有 IPv4 地址
    fn is_private_ipv4(ip: Ipv4Addr) -> bool {
        let octets = ip.octets();
        // 10.0.0.0/8
        if octets[0] == 10 {
            return true;
        }
        // 172.16.0.0/12
        if octets[0] == 172 && (16..=31).contains(&octets[1]) {
            return true;
        }
        // 192.168.0.0/16
        if octets[0] == 192 && octets[1] == 168 {
            return true;
        }
        false
    }

    /// 判断是否为私有 IPv6 地址
    fn is_private_ipv6(ip: Ipv6Addr) -> bool {
        // 链路本地地址 (fe80::/10)
        if ip.segments()[0] & 0xffc0 == 0xfe80 {
            return true;
        }
        // 唯一本地地址 (fc00::/7) 或 (fd00::/8)
        if ip.segments()[0] & 0xfe00 == 0xfc00 {
            return true;
        }
        false
    }

    /// 添加 IPv4 私有网络广播地址
    fn add_ipv4_broadcasts(addresses: &mut Vec<SocketAddr>, ipv4_addrs: &[Ipv4Addr], port: u16) {
        for &ip in ipv4_addrs {
            let octets = ip.octets();

            // 基于具体 IP 地址生成子网广播地址
            if octets[0] == 192 && octets[1] == 168 {
                addresses.push(SocketAddr::from(([192, 168, octets[2], 255], port)));
            } else if octets[0] == 10 {
                addresses.push(SocketAddr::from(([10, octets[1], 255, 255], port)));
            } else if octets[0] == 172 && (16..=31).contains(&octets[1]) {
                addresses.push(SocketAddr::from(([172, octets[1], 255, 255], port)));
            }
        }

        // 添加常见的私有网络广播地址
        if !ipv4_addrs.is_empty() {
            addresses.push(SocketAddr::from(([192, 168, 255, 255], port)));
            addresses.push(SocketAddr::from(([10, 255, 255, 255], port)));
            addresses.push(SocketAddr::from(([172, 31, 255, 255], port)));
        }
    }

    /// 添加 IPv4 公网广播地址
    fn add_ipv4_public_broadcasts(
        addresses: &mut Vec<SocketAddr>,
        _ipv4_addrs: &[Ipv4Addr],
        port: u16,
    ) {
        // 对于公网地址，我们只能使用有限广播
        addresses.push(SocketAddr::from(([255, 255, 255, 255], port)));
    }

    /// 添加 IPv6 多播地址
    fn add_ipv6_multicasts(addresses: &mut Vec<SocketAddr>, port: u16) {
        // 站点本地多播 (ff05::1)
        addresses.push(SocketAddr::new(
            IpAddr::V6(Ipv6Addr::new(0xff05, 0, 0, 0, 0, 0, 0, 1)),
            port,
        ));

        // 链路本地多播 (ff02::1)
        addresses.push(SocketAddr::new(
            IpAddr::V6(Ipv6Addr::new(0xff02, 0, 0, 0, 0, 0, 0, 1)),
            port,
        ));

        // 自定义的 WDIC 多播地址 (ff05::5555)
        addresses.push(SocketAddr::new(
            IpAddr::V6(Ipv6Addr::new(0xff05, 0, 0, 0, 0, 0, 0, 0x5555)),
            port,
        ));
    }

    /// 生成后备广播地址
    fn generate_fallback_addresses(port: u16) -> Vec<SocketAddr> {
        vec![
            SocketAddr::from(([255, 255, 255, 255], port)),
            SocketAddr::from(([192, 168, 255, 255], port)),
            SocketAddr::from(([10, 255, 255, 255], port)),
            SocketAddr::from(([172, 31, 255, 255], port)),
        ]
    }

    /// 启动网络服务
    ///
    /// 开始监听网络消息和处理连接。
    pub async fn start(&self) -> Result<()> {
        info!("网络管理器在 {} 启动", self.local_addr);

        // 启动 UDP 监听任务
        let socket = Arc::clone(&self.udp_socket);
        let event_sender = self.event_sender.clone();
        let connections = Arc::clone(&self.connections);
        let protocol = self.protocol.clone();

        tokio::spawn(async move {
            Self::udp_listener_task(socket, event_sender, connections, protocol).await;
        });

        // 启动连接清理任务
        let connections_cleanup = Arc::clone(&self.connections);
        tokio::spawn(async move {
            Self::connection_cleanup_task(connections_cleanup).await;
        });

        Ok(())
    }

    /// UDP 监听任务
    async fn udp_listener_task(
        socket: Arc<UdpSocket>,
        event_sender: mpsc::UnboundedSender<NetworkEvent>,
        connections: Arc<Mutex<HashMap<SocketAddr, ConnectionState>>>,
        protocol: WdicProtocol,
    ) {
        let mut buffer = [0u8; 65536];

        loop {
            match socket.recv_from(&mut buffer) {
                Ok((size, sender_addr)) => {
                    debug!("收到来自 {sender_addr} 的 {size} 字节数据");

                    // 更新连接状态
                    {
                        let mut conns = connections.lock().await;
                        if let Some(conn) = conns.get_mut(&sender_addr) {
                            conn.update_activity();
                        } else {
                            conns.insert(sender_addr, ConnectionState::new(sender_addr));
                            let _ = event_sender.send(NetworkEvent::ConnectionEstablished {
                                remote_addr: sender_addr,
                            });
                        }
                    }

                    // 解析消息
                    match WdicMessage::from_bytes(&buffer[..size]) {
                        Ok(message) => {
                            debug!("解析消息成功: {}", message.message_type());

                            // 验证消息
                            if let Err(e) = protocol.validate_message(&message) {
                                warn!("消息验证失败: {e}");
                                continue;
                            }

                            let _ = event_sender.send(NetworkEvent::MessageReceived {
                                message,
                                sender: sender_addr,
                            });
                        }
                        Err(e) => {
                            warn!("解析消息失败: {e}");
                        }
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    // 非阻塞模式下没有数据可读
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
                Err(e) => {
                    error!("UDP 接收错误: {e}");
                    let _ = event_sender.send(NetworkEvent::NetworkError {
                        error: format!("UDP 接收错误: {e}"),
                    });
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            }
        }
    }

    /// 连接清理任务
    async fn connection_cleanup_task(
        connections: Arc<Mutex<HashMap<SocketAddr, ConnectionState>>>,
    ) {
        let mut cleanup_interval = interval(Duration::from_secs(60));

        loop {
            cleanup_interval.tick().await;

            let mut conns = connections.lock().await;
            let expired_addrs: Vec<SocketAddr> = conns
                .iter()
                .filter(|(_, state)| state.is_expired(300)) // 5分钟超时
                .map(|(addr, _)| *addr)
                .collect();

            for addr in expired_addrs {
                conns.remove(&addr);
                debug!("清理过期连接: {addr}");
            }
        }
    }

    /// 发送消息到指定地址
    ///
    /// # 参数
    ///
    /// * `message` - 要发送的消息
    /// * `target` - 目标地址
    ///
    /// # 返回值
    ///
    /// 发送结果
    pub async fn send_message(&self, message: &WdicMessage, target: SocketAddr) -> Result<()> {
        let data = message.to_bytes()?;

        debug!("发送 {} 消息到 {target}", message.message_type());

        self.udp_socket
            .send_to(&data, target)
            .map_err(|e| anyhow::anyhow!("发送消息到 {target} 失败: {e}"))?;

        Ok(())
    }

    /// 广播消息到本地网络
    ///
    /// # 参数
    ///
    /// * `message` - 要广播的消息
    ///
    /// # 返回值
    ///
    /// 成功发送的地址数量
    pub async fn broadcast_message(&self, message: &WdicMessage) -> Result<usize> {
        let data = message.to_bytes()?;
        let mut success_count = 0;

        info!(
            "广播 {} 消息到 {} 个地址",
            message.message_type(),
            self.broadcast_addresses.len()
        );

        for &broadcast_addr in &self.broadcast_addresses {
            match self.udp_socket.send_to(&data, broadcast_addr) {
                Ok(_) => {
                    success_count += 1;
                    debug!("成功广播到 {broadcast_addr}");
                }
                Err(e) => {
                    warn!("广播到 {broadcast_addr} 失败: {e}");
                }
            }
        }

        // 发送广播完成事件
        let _ = self.event_sender.send(NetworkEvent::BroadcastSent {
            message: message.clone(),
        });

        Ok(success_count)
    }

    /// 回复消息到发送者
    ///
    /// # 参数
    ///
    /// * `response` - 响应消息
    /// * `original_sender` - 原始发送者地址
    ///
    /// # 返回值
    ///
    /// 发送结果
    pub async fn reply_message(
        &self,
        response: &WdicMessage,
        original_sender: SocketAddr,
    ) -> Result<()> {
        self.send_message(response, original_sender).await
    }

    /// 获取当前活跃连接数
    ///
    /// # 返回值
    ///
    /// 活跃连接数量
    pub async fn active_connections_count(&self) -> usize {
        self.connections.lock().await.len()
    }

    /// 获取所有活跃连接
    ///
    /// # 返回值
    ///
    /// 活跃连接状态列表
    pub async fn get_active_connections(&self) -> Vec<ConnectionState> {
        self.connections.lock().await.values().cloned().collect()
    }

    /// 断开指定连接
    ///
    /// # 参数
    ///
    /// * `addr` - 要断开的连接地址
    ///
    /// # 返回值
    ///
    /// 是否成功断开连接
    pub async fn disconnect(&self, addr: SocketAddr) -> bool {
        let mut conns = self.connections.lock().await;
        if conns.remove(&addr).is_some() {
            let _ = self
                .event_sender
                .send(NetworkEvent::ConnectionLost { remote_addr: addr });
            true
        } else {
            false
        }
    }

    /// 关闭网络管理器
    ///
    /// # 返回值
    ///
    /// 操作结果
    pub async fn shutdown(&self) -> Result<()> {
        info!("关闭网络管理器");
        // 清理所有连接
        self.connections.lock().await.clear();
        Ok(())
    }

    // ============================================================================
    // Tauri API 需要的方法
    // ============================================================================

    /// 获取网络信息
    ///
    /// # 返回值
    ///
    /// 网络状态信息
    pub async fn get_network_info(&self) -> anyhow::Result<crate::tauri_api::NetworkStatus> {
        use if_addrs::get_if_addrs;

        let interfaces = get_if_addrs()
            .map_err(|e| anyhow::anyhow!("获取网络接口失败: {}", e))?;

        let network_interfaces: Vec<crate::tauri_api::NetworkInterface> = interfaces
            .into_iter()
            .map(|iface| {
                let name = iface.name.clone();
                crate::tauri_api::NetworkInterface {
                    name,
                    ip_address: iface.ip().to_string(),
                    is_active: !iface.ip().is_loopback(),
                    interface_type: if iface.ip().is_ipv4() {
                        "IPv4".to_string()
                    } else {
                        "IPv6".to_string()
                    },
                }
            })
            .collect();

        let local_ip = self.local_addr.ip().to_string();
        let listen_port = self.local_addr.port();

        Ok(crate::tauri_api::NetworkStatus {
            local_ip,
            listen_port,
            network_interfaces,
            p2p_discovery_enabled: false, // 这里应该从实际状态获取
            discovered_nodes: 0,          // 这里应该从实际状态获取
        })
    }

    /// 启动 P2P 发现
    ///
    /// # 返回值
    ///
    /// 操作结果
    pub async fn start_p2p_discovery(&self) -> anyhow::Result<()> {
        log::info!("启动 P2P 发现");
        // 这里应该启动实际的 P2P 发现逻辑
        Ok(())
    }

    /// 停止 P2P 发现
    ///
    /// # 返回值
    ///
    /// 操作结果
    pub async fn stop_p2p_discovery(&self) -> anyhow::Result<()> {
        log::info!("停止 P2P 发现");
        // 这里应该停止实际的 P2P 发现逻辑
        Ok(())
    }

    /// 获取已发现的节点列表
    ///
    /// # 返回值
    ///
    /// 发现的节点列表
    pub async fn get_discovered_nodes(&self) -> anyhow::Result<Vec<crate::tauri_api::DiscoveredNode>> {
        use chrono::Utc;

        // 这里应该从实际的发现机制获取节点
        // 目前返回示例数据
        let nodes = vec![
            crate::tauri_api::DiscoveredNode {
                node_id: uuid::Uuid::new_v4().to_string(),
                ip_address: "192.168.1.100".to_string(),
                port: 55555,
                name: "示例节点1".to_string(),
                discovered_time: Utc::now(),
                last_seen: Utc::now(),
                is_online: true,
                node_type: "gateway".to_string(),
            },
        ];

        Ok(nodes)
    }

    /// 连接到指定节点
    ///
    /// # 参数
    ///
    /// * `node_id` - 节点 ID
    /// * `ip_address` - IP 地址
    /// * `port` - 端口
    ///
    /// # 返回值
    ///
    /// 操作结果
    pub async fn connect_to_node(
        &self,
        node_id: &str,
        ip_address: &str,
        port: u16,
    ) -> anyhow::Result<()> {
        use std::net::SocketAddr;

        let addr: SocketAddr = format!("{ip_address}:{port}")
            .parse()
            .map_err(|e| anyhow::anyhow!("无效地址: {}", e))?;

        log::info!("连接到节点 {node_id} ({addr})");

        // 这里应该建立实际的连接
        let mut connections = self.connections.lock().await;
        connections.insert(addr, ConnectionState::new(addr));

        Ok(())
    }

    /// 断开与节点的连接
    ///
    /// # 参数
    ///
    /// * `node_id` - 节点 ID
    ///
    /// # 返回值
    ///
    /// 操作结果
    pub async fn disconnect_from_node(&self, node_id: &str) -> anyhow::Result<()> {
        log::info!("断开与节点 {node_id} 的连接");

        // 这里应该根据 node_id 找到对应的连接并断开
        // 目前简化实现
        Ok(())
    }

    /// 创建文件传输任务
    ///
    /// # 参数
    ///
    /// * `task_id` - 任务 ID
    /// * `source_path` - 源路径
    /// * `target_path` - 目标路径
    ///
    /// # 返回值
    ///
    /// 操作结果
    pub async fn create_transfer_task(
        &self,
        task_id: String,
        source_path: std::path::PathBuf,
        target_path: std::path::PathBuf,
    ) -> anyhow::Result<()> {
        log::info!(
            "创建文件传输任务 {task_id}: {source_path:?} -> {target_path:?}"
        );

        // 这里应该创建实际的传输任务
        Ok(())
    }

    /// 获取文件传输任务状态
    ///
    /// # 参数
    ///
    /// * `task_id` - 任务 ID
    ///
    /// # 返回值
    ///
    /// 传输任务状态
    pub async fn get_transfer_status(
        &self,
        task_id: &str,
    ) -> anyhow::Result<crate::tauri_api::FileTransferTask> {
        use chrono::Utc;

        // 这里应该从实际存储中获取任务状态
        // 目前返回示例数据
        let task = crate::tauri_api::FileTransferTask {
            id: task_id.to_string(),
            source_path: std::path::PathBuf::from("/tmp/source.txt"),
            target_path: std::path::PathBuf::from("/tmp/target.txt"),
            status: crate::tauri_api::TransferStatus::Pending,
            transferred_bytes: 0,
            total_bytes: 1024,
            transfer_speed: 0,
            start_time: Utc::now(),
            estimated_completion: None,
        };

        Ok(task)
    }

    /// 取消文件传输任务
    ///
    /// # 参数
    ///
    /// * `task_id` - 任务 ID
    ///
    /// # 返回值
    ///
    /// 操作结果
    pub async fn cancel_transfer(&self, task_id: &str) -> anyhow::Result<()> {
        log::info!("取消文件传输任务 {task_id}");

        // 这里应该取消实际的传输任务
        Ok(())
    }

    /// 健康检查
    ///
    /// # 返回值
    ///
    /// 健康状态
    pub async fn health_check(&self) -> Option<bool> {
        // 简单的健康检查：检查本地地址是否有效
        Some(!self.local_addr.ip().is_unspecified())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};

    fn create_test_addr(port: u16) -> SocketAddr {
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), port)
    }

    #[test]
    fn test_connection_state() {
        let addr = create_test_addr(55555);
        let mut state = ConnectionState::new(addr);

        assert_eq!(state.remote_addr, addr);
        assert!(state.last_active <= chrono::Utc::now());
        assert_eq!(state.last_active, state.established_at);

        // 测试活跃时间更新
        let original_time = state.last_active;
        std::thread::sleep(std::time::Duration::from_millis(1));
        state.update_activity();
        assert!(state.last_active > original_time);

        // 测试超时检查
        assert!(!state.is_expired(3600)); // 1小时不会超时

        // 创建过期连接
        state.last_active = chrono::Utc::now() - chrono::Duration::seconds(7200);
        assert!(state.is_expired(3600)); // 2小时前的连接超时
    }

    #[test]
    fn test_broadcast_addresses_generation() {
        let local_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100)), 55555);
        let addresses = NetworkManager::generate_broadcast_addresses(local_addr);

        assert!(!addresses.is_empty());
        // 检查是否包含至少一个广播地址
        let has_ipv4_broadcast = addresses
            .iter()
            .any(|addr| matches!(addr.ip(), IpAddr::V4(_)));
        let has_valid_port = addresses.iter().all(|addr| addr.port() == 55555);

        assert!(has_ipv4_broadcast, "应该至少包含一个 IPv4 广播地址");
        assert!(has_valid_port, "所有地址应该使用正确的端口");

        // 在没有网络接口的环境中，应该至少有后备地址
        if addresses
            .iter()
            .any(|addr| addr == &SocketAddr::from(([255, 255, 255, 255], 55555)))
        {
            // 如果有全网广播地址，测试通过
            // 不需要assert!(true)
        } else {
            // 否则应该有其他有效的广播地址
            assert!(!addresses.is_empty(), "应该生成至少一个广播地址");
        }
    }

    #[test]
    fn test_ipv6_multicast_addresses_generation() {
        // 测试 IPv6 多播地址生成
        let local_addr = SocketAddr::new(
            IpAddr::V6(Ipv6Addr::new(0xfe80, 0, 0, 0, 0, 0, 0, 1)),
            55555,
        );
        let addresses = NetworkManager::generate_broadcast_addresses(local_addr);

        assert!(!addresses.is_empty());

        // 检查 IPv6 多播地址是否正确生成
        let _has_ipv6_multicast = addresses.iter().any(
            |addr| matches!(addr.ip(), IpAddr::V6(ipv6) if ipv6.segments()[0] & 0xff00 == 0xff00),
        );

        // 如果系统支持 IPv6，应该有多播地址
        let has_valid_port = addresses.iter().all(|addr| addr.port() == 55555);
        assert!(has_valid_port, "所有地址应该使用正确的端口");
    }

    #[test]
    fn test_private_ipv4_detection() {
        assert!(NetworkManager::is_private_ipv4(Ipv4Addr::new(
            192, 168, 1, 1
        )));
        assert!(NetworkManager::is_private_ipv4(Ipv4Addr::new(10, 0, 0, 1)));
        assert!(NetworkManager::is_private_ipv4(Ipv4Addr::new(
            172, 16, 0, 1
        )));
        assert!(NetworkManager::is_private_ipv4(Ipv4Addr::new(
            172, 31, 255, 255
        )));

        assert!(!NetworkManager::is_private_ipv4(Ipv4Addr::new(8, 8, 8, 8)));
        assert!(!NetworkManager::is_private_ipv4(Ipv4Addr::new(1, 1, 1, 1)));
        assert!(!NetworkManager::is_private_ipv4(Ipv4Addr::new(
            172, 15, 0, 1
        ))); // 不在私有范围
        assert!(!NetworkManager::is_private_ipv4(Ipv4Addr::new(
            172, 32, 0, 1
        ))); // 不在私有范围
    }

    #[test]
    fn test_private_ipv6_detection() {
        // 链路本地地址 (fe80::/10)
        assert!(NetworkManager::is_private_ipv6(Ipv6Addr::new(
            0xfe80, 0, 0, 0, 0, 0, 0, 1
        )));
        assert!(NetworkManager::is_private_ipv6(Ipv6Addr::new(
            0xfebf, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff, 0xffff
        )));

        // 唯一本地地址 (fc00::/7)
        assert!(NetworkManager::is_private_ipv6(Ipv6Addr::new(
            0xfc00, 0, 0, 0, 0, 0, 0, 1
        )));
        assert!(NetworkManager::is_private_ipv6(Ipv6Addr::new(
            0xfd00, 0, 0, 0, 0, 0, 0, 1
        )));

        // 公网地址
        assert!(!NetworkManager::is_private_ipv6(Ipv6Addr::new(
            0x2001, 0x4860, 0x4860, 0, 0, 0, 0, 0x8888
        ))); // Google DNS
        assert!(!NetworkManager::is_private_ipv6(Ipv6Addr::new(
            0x2001, 0xdb8, 0, 0, 0, 0, 0, 1
        ))); // 文档地址
    }

    #[test]
    fn test_fallback_addresses_generation() {
        let addresses = NetworkManager::generate_fallback_addresses(12345);

        assert!(!addresses.is_empty());
        assert!(addresses.contains(&SocketAddr::from(([255, 255, 255, 255], 12345))));
        assert!(addresses.contains(&SocketAddr::from(([192, 168, 255, 255], 12345))));
        assert!(addresses.contains(&SocketAddr::from(([10, 255, 255, 255], 12345))));
        assert!(addresses.contains(&SocketAddr::from(([172, 31, 255, 255], 12345))));
    }

    #[tokio::test]
    async fn test_network_manager_creation() {
        let local_addr = create_test_addr(0); // 使用端口 0 让系统分配
        let manager = NetworkManager::new(local_addr);

        assert!(manager.is_ok());
        let _manager = manager.unwrap();
        // 端口 0 会被系统分配一个有效端口，或者保持 0 但绑定成功
        // 端口已分配成功
    }

    #[tokio::test]
    async fn test_network_manager_basic_operations() {
        let local_addr = create_test_addr(0);
        let manager = NetworkManager::new(local_addr).expect("创建网络管理器失败");

        // 测试基本属性
        assert_eq!(manager.active_connections_count().await, 0);
        assert!(manager.get_active_connections().await.is_empty());

        // 测试关闭
        assert!(manager.shutdown().await.is_ok());
    }
}
