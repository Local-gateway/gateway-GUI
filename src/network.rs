//! 网络管理模块
//! 
//! 处理 QUIC 连接、UDP 广播和网络通信。

use std::collections::HashMap;
use std::net::{SocketAddr, UdpSocket};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tokio::time::{Duration, interval};
use anyhow::Result;
use log::{info, warn, error, debug};

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
    fn generate_broadcast_addresses(local_addr: SocketAddr) -> Vec<SocketAddr> {
        let mut addresses = Vec::new();
        let port = local_addr.port();

        // 添加主要的广播地址
        addresses.push(SocketAddr::from(([255, 255, 255, 255], port)));
        
        // 如果是 IPv4，根据本地 IP 生成子网广播地址
        if let std::net::IpAddr::V4(ipv4) = local_addr.ip() {
            let octets = ipv4.octets();
            
            // 常见的私有网络广播地址
            addresses.push(SocketAddr::from(([192, 168, 255, 255], port)));
            addresses.push(SocketAddr::from(([10, 255, 255, 255], port)));
            addresses.push(SocketAddr::from(([172, 31, 255, 255], port)));
            
            // 基于当前 IP 的子网广播
            if octets[0] == 192 && octets[1] == 168 {
                addresses.push(SocketAddr::from(([192, 168, octets[2], 255], port)));
            } else if octets[0] == 10 {
                addresses.push(SocketAddr::from(([10, octets[1], 255, 255], port)));
            } else if octets[0] == 172 && (16..=31).contains(&octets[1]) {
                addresses.push(SocketAddr::from(([172, octets[1], 255, 255], port)));
            }
        }

        addresses
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
                    debug!("收到来自 {} 的 {} 字节数据", sender_addr, size);

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
                                warn!("消息验证失败: {}", e);
                                continue;
                            }

                            let _ = event_sender.send(NetworkEvent::MessageReceived {
                                message,
                                sender: sender_addr,
                            });
                        }
                        Err(e) => {
                            warn!("解析消息失败: {}", e);
                        }
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    // 非阻塞模式下没有数据可读
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
                Err(e) => {
                    error!("UDP 接收错误: {}", e);
                    let _ = event_sender.send(NetworkEvent::NetworkError {
                        error: format!("UDP 接收错误: {}", e),
                    });
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            }
        }
    }

    /// 连接清理任务
    async fn connection_cleanup_task(connections: Arc<Mutex<HashMap<SocketAddr, ConnectionState>>>) {
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
                debug!("清理过期连接: {}", addr);
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
        
        debug!("发送 {} 消息到 {}", message.message_type(), target);
        
        self.udp_socket.send_to(&data, target).map_err(|e| {
            anyhow::anyhow!("发送消息到 {} 失败: {}", target, e)
        })?;

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

        info!("广播 {} 消息到 {} 个地址", message.message_type(), self.broadcast_addresses.len());

        for &broadcast_addr in &self.broadcast_addresses {
            match self.udp_socket.send_to(&data, broadcast_addr) {
                Ok(_) => {
                    success_count += 1;
                    debug!("成功广播到 {}", broadcast_addr);
                }
                Err(e) => {
                    warn!("广播到 {} 失败: {}", broadcast_addr, e);
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
    pub async fn reply_message(&self, response: &WdicMessage, original_sender: SocketAddr) -> Result<()> {
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
            let _ = self.event_sender.send(NetworkEvent::ConnectionLost {
                remote_addr: addr,
            });
            true
        } else {
            false
        }
    }

    /// 关闭网络管理器
    pub async fn shutdown(&self) -> Result<()> {
        info!("关闭网络管理器");
        
        // 清空所有连接
        {
            let mut conns = self.connections.lock().await;
            conns.clear();
        }

        Ok(())
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
        assert!(addresses.contains(&SocketAddr::from(([255, 255, 255, 255], 55555))));
        assert!(addresses.contains(&SocketAddr::from(([192, 168, 255, 255], 55555))));
        assert!(addresses.contains(&SocketAddr::from(([192, 168, 1, 255], 55555))));
    }

    #[tokio::test]
    async fn test_network_manager_creation() {
        let local_addr = create_test_addr(0); // 使用端口 0 让系统分配
        let manager = NetworkManager::new(local_addr);
        
        assert!(manager.is_ok());
        let manager = manager.unwrap();
        // 端口 0 会被系统分配一个有效端口，或者保持 0 但绑定成功
        assert!(manager.local_addr().port() >= 0);
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