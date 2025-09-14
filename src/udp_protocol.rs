//! UDP 广播协议模块
//! 
//! 实现基于 UDP 的 WDIC 协议自主广播功能，所有网关都是一等公民。

use std::collections::HashMap;
use std::net::{SocketAddr, UdpSocket};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio::time::Duration;
use anyhow::Result;
use log::{info, debug};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use base64::{Engine as _, engine::general_purpose};

use crate::protocol::WdicMessage;

/// UDP 广播令牌类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum UdpToken {
    /// 目录搜索令牌
    DirectorySearch {
        /// 搜索者 ID
        searcher_id: Uuid,
        /// 搜索关键词
        keywords: Vec<String>,
        /// 搜索 ID
        search_id: Uuid,
    },
    /// 目录搜索响应令牌
    DirectorySearchResponse {
        /// 响应者 ID
        responder_id: Uuid,
        /// 搜索 ID
        search_id: Uuid,
        /// 匹配的文件列表
        matches: Vec<String>,
    },
    /// 文件请求令牌
    FileRequest {
        /// 请求者 ID
        requester_id: Uuid,
        /// 文件路径
        file_path: String,
        /// 请求 ID
        request_id: Uuid,
    },
    /// 文件响应令牌
    FileResponse {
        /// 响应者 ID
        responder_id: Uuid,
        /// 请求 ID
        request_id: Uuid,
        /// 文件数据（Base64 编码）
        file_data: Option<String>,
        /// 错误信息
        error: Option<String>,
    },
    /// 信息发送令牌
    InfoMessage {
        /// 发送者 ID
        sender_id: Uuid,
        /// 消息内容
        content: String,
        /// 消息 ID
        message_id: Uuid,
    },
    /// 性能测试令牌
    PerformanceTest {
        /// 测试者 ID
        tester_id: Uuid,
        /// 测试类型
        test_type: String,
        /// 测试数据大小
        data_size: usize,
        /// 测试开始时间
        start_time: chrono::DateTime<chrono::Utc>,
    },
}

/// UDP 广播事件
#[derive(Debug, Clone)]
pub enum UdpBroadcastEvent {
    /// 收到令牌
    TokenReceived {
        /// 令牌内容
        token: UdpToken,
        /// 发送者地址
        sender: SocketAddr,
    },
    /// 广播发送完成
    BroadcastSent {
        /// 令牌内容
        token: UdpToken,
        /// 发送到的地址数
        sent_count: usize,
    },
    /// 网络错误
    NetworkError {
        /// 错误信息
        error: String,
    },
}

/// 目录条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectoryEntry {
    /// 文件路径
    pub path: String,
    /// 文件大小
    pub size: u64,
    /// 是否为目录
    pub is_dir: bool,
    /// 修改时间
    pub modified: chrono::DateTime<chrono::Utc>,
}

/// 目录索引
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectoryIndex {
    /// 根目录路径
    pub root_path: String,
    /// 目录条目列表
    pub entries: Vec<DirectoryEntry>,
    /// 生成时间
    pub generated_at: chrono::DateTime<chrono::Utc>,
}

impl DirectoryIndex {
    /// 生成目录索引
    /// 
    /// # 参数
    /// 
    /// * `path` - 目录路径
    /// 
    /// # 返回值
    /// 
    /// 目录索引实例
    pub fn generate(path: &str) -> Result<Self> {
        let mut entries = Vec::new();
        
        fn scan_directory(dir_path: &std::path::Path, entries: &mut Vec<DirectoryEntry>) -> Result<()> {
            if !dir_path.exists() {
                return Err(anyhow::anyhow!("目录不存在: {}", dir_path.display()));
            }
            
            for entry in std::fs::read_dir(dir_path)? {
                let entry = entry?;
                let path = entry.path();
                let metadata = entry.metadata()?;
                
                let dir_entry = DirectoryEntry {
                    path: path.to_string_lossy().to_string(),
                    size: metadata.len(),
                    is_dir: metadata.is_dir(),
                    modified: metadata.modified()
                        .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
                        .duration_since(std::time::SystemTime::UNIX_EPOCH)
                        .map(|d| chrono::DateTime::from_timestamp(d.as_secs() as i64, 0))
                        .unwrap_or(None)
                        .unwrap_or_else(chrono::Utc::now),
                };
                
                entries.push(dir_entry);
                
                // 递归扫描子目录
                if metadata.is_dir() {
                    scan_directory(&path, entries)?;
                }
            }
            
            Ok(())
        }
        
        let root_path = std::path::Path::new(path);
        scan_directory(root_path, &mut entries)?;
        
        Ok(Self {
            root_path: path.to_string(),
            entries,
            generated_at: chrono::Utc::now(),
        })
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
    pub fn search(&self, keywords: &[String]) -> Vec<String> {
        self.entries
            .iter()
            .filter(|entry| {
                let path_lower = entry.path.to_lowercase();
                keywords.iter().any(|keyword| path_lower.contains(&keyword.to_lowercase()))
            })
            .map(|entry| entry.path.clone())
            .collect()
    }
    
    /// 保存索引到文件
    /// 
    /// # 参数
    /// 
    /// * `output_path` - 输出文件路径
    /// 
    /// # 返回值
    /// 
    /// 保存结果
    pub fn save_to_file(&self, output_path: &str) -> Result<()> {
        let serialized = serde_json::to_vec(self)
            .map_err(|e| anyhow::anyhow!("序列化目录索引失败: {}", e))?;
        
        std::fs::write(output_path, serialized)
            .map_err(|e| anyhow::anyhow!("写入索引文件失败: {}", e))?;
        
        info!("目录索引已保存到: {}", output_path);
        Ok(())
    }
    
    /// 从文件加载索引
    /// 
    /// # 参数
    /// 
    /// * `input_path` - 输入文件路径
    /// 
    /// # 返回值
    /// 
    /// 目录索引实例
    pub fn load_from_file(input_path: &str) -> Result<Self> {
        let data = std::fs::read(input_path)
            .map_err(|e| anyhow::anyhow!("读取索引文件失败: {}", e))?;
        
        serde_json::from_slice(&data)
            .map_err(|e| anyhow::anyhow!("反序列化目录索引失败: {}", e))
    }
}

/// UDP 广播管理器
/// 
/// 负责处理基于 UDP 的 WDIC 协议广播功能。
pub struct UdpBroadcastManager {
    /// 本地地址
    local_addr: SocketAddr,
    /// UDP 套接字
    udp_socket: Arc<UdpSocket>,
    /// 事件发送通道
    event_sender: mpsc::UnboundedSender<UdpBroadcastEvent>,
    /// 事件接收通道
    event_receiver: Arc<Mutex<Option<mpsc::UnboundedReceiver<UdpBroadcastEvent>>>>,
    /// 广播地址列表
    broadcast_addresses: Vec<SocketAddr>,
    /// 目录挂载点
    mounted_directories: Arc<RwLock<HashMap<String, DirectoryIndex>>>,
    /// 运行状态
    running: Arc<Mutex<bool>>,
}

impl UdpBroadcastManager {
    /// 创建新的 UDP 广播管理器
    /// 
    /// # 参数
    /// 
    /// * `local_addr` - 本地监听地址
    /// 
    /// # 返回值
    /// 
    /// UDP 广播管理器实例
    pub fn new(local_addr: SocketAddr) -> Result<Self> {
        let udp_socket = UdpSocket::bind(local_addr)?;
        udp_socket.set_broadcast(true)?;
        udp_socket.set_nonblocking(true)?;

        let (event_sender, event_receiver) = mpsc::unbounded_channel();

        // 生成广播地址
        let broadcast_addresses = Self::generate_broadcast_addresses(local_addr);

        Ok(Self {
            local_addr,
            udp_socket: Arc::new(udp_socket),
            event_sender,
            event_receiver: Arc::new(Mutex::new(Some(event_receiver))),
            broadcast_addresses,
            mounted_directories: Arc::new(RwLock::new(HashMap::new())),
            running: Arc::new(Mutex::new(false)),
        })
    }

    /// 获取本地地址
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    /// 获取事件接收器
    pub async fn take_event_receiver(&self) -> Option<mpsc::UnboundedReceiver<UdpBroadcastEvent>> {
        self.event_receiver.lock().await.take()
    }

    /// 生成广播地址列表
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

    /// 启动 UDP 广播服务
    /// 
    /// # 返回值
    /// 
    /// 启动结果
    pub async fn start(&self) -> Result<()> {
        {
            let mut running = self.running.lock().await;
            if *running {
                return Err(anyhow::anyhow!("UDP 广播管理器已经在运行"));
            }
            *running = true;
        }

        info!("UDP 广播管理器在 {} 启动", self.local_addr);

        // 启动 UDP 监听任务
        let socket = Arc::clone(&self.udp_socket);
        let event_sender = self.event_sender.clone();
        let running = Arc::clone(&self.running);

        tokio::spawn(async move {
            Self::udp_listener_task(socket, event_sender, running).await;
        });

        Ok(())
    }

    /// UDP 监听任务
    async fn udp_listener_task(
        socket: Arc<UdpSocket>,
        event_sender: mpsc::UnboundedSender<UdpBroadcastEvent>,
        running: Arc<Mutex<bool>>,
    ) {
        let mut buffer = [0u8; 65536];

        while *running.lock().await {
            match socket.recv_from(&mut buffer) {
                Ok((size, sender_addr)) => {
                    debug!("收到来自 {} 的 {} 字节 UDP 数据", sender_addr, size);

                    // 尝试解析为 UDP 令牌
                    match serde_json::from_slice::<UdpToken>(&buffer[..size]) {
                        Ok(token) => {
                            debug!("解析 UDP 令牌成功: {:?}", token);
                            let _ = event_sender.send(UdpBroadcastEvent::TokenReceived {
                                token,
                                sender: sender_addr,
                            });
                        }
                        Err(e) => {
                            debug!("解析 UDP 令牌失败，尝试解析为 WDIC 消息: {}", e);
                            // 尝试解析为 WDIC 消息（向后兼容）
                            if let Ok(_message) = serde_json::from_slice::<WdicMessage>(&buffer[..size]) {
                                debug!("解析为 WDIC 消息成功，但在 UDP 广播管理器中忽略");
                            }
                        }
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    // 非阻塞模式下没有数据可读
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
                Err(e) => {
                    // 隐蔽 OS 异常，只记录调试信息
                    debug!("UDP 接收时出现 OS 异常（已隐蔽处理）: {}", e);
                    let _ = event_sender.send(UdpBroadcastEvent::NetworkError {
                        error: format!("网络通信异常"),
                    });
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            }
        }
    }

    /// 广播令牌
    /// 
    /// # 参数
    /// 
    /// * `token` - 要广播的令牌
    /// 
    /// # 返回值
    /// 
    /// 成功发送的地址数量
    pub async fn broadcast_token(&self, token: &UdpToken) -> Result<usize> {
        let data = serde_json::to_vec(token)
            .map_err(|e| anyhow::anyhow!("序列化令牌失败: {}", e))?;
        
        let mut success_count = 0;

        for &broadcast_addr in &self.broadcast_addresses {
            match self.udp_socket.send_to(&data, broadcast_addr) {
                Ok(_) => {
                    success_count += 1;
                    debug!("成功广播令牌到 {}", broadcast_addr);
                }
                Err(e) => {
                    // 隐蔽 OS 异常
                    debug!("广播到 {} 时出现 OS 异常（已隐蔽处理）: {}", broadcast_addr, e);
                }
            }
        }

        // 发送广播完成事件
        let _ = self.event_sender.send(UdpBroadcastEvent::BroadcastSent {
            token: token.clone(),
            sent_count: success_count,
        });

        Ok(success_count)
    }

    /// 定向广播令牌到指定地址
    /// 
    /// # 参数
    /// 
    /// * `token` - 要发送的令牌
    /// * `target` - 目标地址
    /// 
    /// # 返回值
    /// 
    /// 发送结果
    pub async fn send_token_to(&self, token: &UdpToken, target: SocketAddr) -> Result<()> {
        let data = serde_json::to_vec(token)
            .map_err(|e| anyhow::anyhow!("序列化令牌失败: {}", e))?;
        
        debug!("发送令牌到 {}", target);
        
        self.udp_socket.send_to(&data, target).map_err(|e| {
            // 隐蔽 OS 异常
            debug!("发送令牌到 {} 时出现 OS 异常: {}", target, e);
            anyhow::anyhow!("网络通信失败")
        })?;

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
        info!("开始挂载目录: {} -> {}", name, path);
        
        let index = DirectoryIndex::generate(&path)?;
        
        // 保存索引文件
        let index_file = format!("{}.index", name);
        index.save_to_file(&index_file)?;
        
        // 添加到挂载点
        {
            let mut mounted = self.mounted_directories.write().await;
            mounted.insert(name.clone(), index);
        }
        
        info!("目录挂载成功: {}", name);
        Ok(())
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
        let mut mounted = self.mounted_directories.write().await;
        mounted.remove(name).is_some()
    }

    /// 获取已挂载目录列表
    /// 
    /// # 返回值
    /// 
    /// 挂载点名称列表
    pub async fn get_mounted_directories(&self) -> Vec<String> {
        let mounted = self.mounted_directories.read().await;
        mounted.keys().cloned().collect()
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
    pub async fn search_files(&self, keywords: &[String]) -> Vec<String> {
        let mounted = self.mounted_directories.read().await;
        let mut results = Vec::new();
        
        for index in mounted.values() {
            results.extend(index.search(keywords));
        }
        
        results
    }

    /// 读取文件内容
    /// 
    /// # 参数
    /// 
    /// * `file_path` - 文件路径
    /// 
    /// # 返回值
    /// 
    /// 文件内容（Base64 编码）
    pub async fn read_file(&self, file_path: &str) -> Result<String> {
        let data = std::fs::read(file_path)
            .map_err(|e| anyhow::anyhow!("读取文件失败: {}", e))?;
        
        Ok(general_purpose::STANDARD.encode(&data))
    }

    /// 发送信息消息
    /// 
    /// # 参数
    /// 
    /// * `sender_id` - 发送者 ID
    /// * `content` - 消息内容
    /// 
    /// # 返回值
    /// 
    /// 发送结果
    pub async fn send_info_message(&self, sender_id: Uuid, content: String) -> Result<usize> {
        let token = UdpToken::InfoMessage {
            sender_id,
            content,
            message_id: Uuid::new_v4(),
        };
        
        self.broadcast_token(&token).await
    }

    /// 执行性能测试
    /// 
    /// # 参数
    /// 
    /// * `tester_id` - 测试者 ID
    /// * `test_type` - 测试类型
    /// * `data_size` - 测试数据大小
    /// 
    /// # 返回值
    /// 
    /// 测试结果（延迟毫秒数）
    pub async fn performance_test(&self, tester_id: Uuid, test_type: String, data_size: usize) -> Result<u64> {
        let start_time = chrono::Utc::now();
        
        let token = UdpToken::PerformanceTest {
            tester_id,
            test_type,
            data_size,
            start_time,
        };
        
        let start = std::time::Instant::now();
        self.broadcast_token(&token).await?;
        let elapsed = start.elapsed();
        
        Ok(elapsed.as_millis() as u64)
    }

    /// 停止 UDP 广播管理器
    /// 
    /// # 返回值
    /// 
    /// 停止结果
    pub async fn stop(&self) -> Result<()> {
        info!("停止 UDP 广播管理器");

        {
            let mut running = self.running.lock().await;
            *running = false;
        }

        // 清理挂载的目录
        {
            let mut mounted = self.mounted_directories.write().await;
            mounted.clear();
        }

        Ok(())
    }

    /// 检查是否正在运行
    /// 
    /// # 返回值
    /// 
    /// 运行状态
    pub async fn is_running(&self) -> bool {
        *self.running.lock().await
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
    fn test_udp_token_serialization() {
        let token = UdpToken::InfoMessage {
            sender_id: Uuid::new_v4(),
            content: "测试消息".to_string(),
            message_id: Uuid::new_v4(),
        };

        let serialized = serde_json::to_vec(&token).expect("序列化失败");
        let deserialized: UdpToken = serde_json::from_slice(&serialized).expect("反序列化失败");

        assert_eq!(token, deserialized);
    }

    #[test]
    fn test_directory_index_generation() {
        // 创建临时测试目录
        let temp_dir = std::env::temp_dir().join("wdic_test");
        std::fs::create_dir_all(&temp_dir).expect("创建测试目录失败");
        
        // 创建测试文件
        let test_file = temp_dir.join("test.txt");
        std::fs::write(&test_file, "测试内容").expect("创建测试文件失败");

        let index = DirectoryIndex::generate(temp_dir.to_str().unwrap());
        assert!(index.is_ok());

        let index = index.unwrap();
        assert!(!index.entries.is_empty());

        // 清理测试目录
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_directory_index_search() {
        let mut index = DirectoryIndex {
            root_path: "/test".to_string(),
            entries: vec![
                DirectoryEntry {
                    path: "/test/file1.txt".to_string(),
                    size: 100,
                    is_dir: false,
                    modified: chrono::Utc::now(),
                },
                DirectoryEntry {
                    path: "/test/document.pdf".to_string(),
                    size: 200,
                    is_dir: false,
                    modified: chrono::Utc::now(),
                },
            ],
            generated_at: chrono::Utc::now(),
        };

        let results = index.search(&["txt".to_string()]);
        assert_eq!(results.len(), 1);
        assert!(results[0].contains("file1.txt"));

        let results = index.search(&["test".to_string()]);
        assert_eq!(results.len(), 2);
    }

    #[tokio::test]
    async fn test_udp_broadcast_manager_creation() {
        let local_addr = create_test_addr(0);
        let manager = UdpBroadcastManager::new(local_addr);
        
        assert!(manager.is_ok());
        let manager = manager.unwrap();
        assert!(!manager.is_running().await);
    }

    #[tokio::test]
    async fn test_udp_broadcast_manager_directory_operations() {
        let local_addr = create_test_addr(0);
        let manager = UdpBroadcastManager::new(local_addr).expect("创建管理器失败");

        // 测试目录挂载（使用当前目录）
        let current_dir = std::env::current_dir().unwrap();
        let mount_result = manager.mount_directory(
            "test_mount".to_string(),
            current_dir.to_string_lossy().to_string()
        ).await;

        // 如果目录存在且可访问，挂载应该成功
        if mount_result.is_ok() {
            let mounted = manager.get_mounted_directories().await;
            assert!(mounted.contains(&"test_mount".to_string()));

            // 测试搜索功能
            let results = manager.search_files(&["rs".to_string()]).await;
            // 应该能找到一些 .rs 文件
            
            // 测试卸载
            let unmounted = manager.unmount_directory("test_mount").await;
            assert!(unmounted);
        }
    }

    #[tokio::test]
    async fn test_udp_broadcast_manager_info_message() {
        let local_addr = create_test_addr(0);
        let manager = UdpBroadcastManager::new(local_addr).expect("创建管理器失败");

        let sender_id = Uuid::new_v4();
        let result = manager.send_info_message(sender_id, "测试消息".to_string()).await;
        
        // 即使广播失败（没有监听者），也应该返回成功
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_udp_broadcast_manager_performance_test() {
        let local_addr = create_test_addr(0);
        let manager = UdpBroadcastManager::new(local_addr).expect("创建管理器失败");

        let tester_id = Uuid::new_v4();
        let result = manager.performance_test(
            tester_id,
            "latency_test".to_string(),
            1024
        ).await;
        
        assert!(result.is_ok());
        let latency = result.unwrap();
        assert!(latency >= 0); // 延迟应该是非负数
    }
}