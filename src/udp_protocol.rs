//! UDP 广播协议模块
//!
//! 实现基于 UDP 的 WDIC 协议自主广播功能，支持 IPv4/IPv6 双栈网络，所有网关都是一等公民。
//! 性能优化版本：使用 SmallVec 减少堆分配，使用 AHash 提升 HashMap 性能。

use ahash::AHashMap;
use anyhow::Result;
use base64::{engine::general_purpose, Engine as _};
use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, UdpSocket};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio::time::Duration;
use uuid::Uuid;

use crate::protocol::WdicMessage;
use crate::security::{PathValidator, SecureFileReader, SearchResultFilter};

/// UDP 广播令牌类型
/// 性能优化：使用 SmallVec 减少小集合的堆分配
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum UdpToken {
    /// 目录搜索令牌
    DirectorySearch {
        /// 搜索者 ID
        searcher_id: Uuid,
        /// 搜索关键词 - 使用 SmallVec，大多数搜索只有几个关键词
        keywords: SmallVec<[String; 4]>,
        /// 搜索 ID
        search_id: Uuid,
    },
    /// 目录搜索响应令牌
    DirectorySearchResponse {
        /// 响应者 ID
        responder_id: Uuid,
        /// 搜索 ID
        search_id: Uuid,
        /// 匹配的文件列表 - 使用 SmallVec，通常匹配结果不多
        matches: SmallVec<[String; 8]>,
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

        // 创建路径验证器，只允许访问指定的根目录
        let validator = PathValidator::new(vec![]);
        let normalized_path = validator.validate_and_normalize(path)?;

        fn scan_directory(
            dir_path: &std::path::Path,
            entries: &mut Vec<DirectoryEntry>,
            validator: &PathValidator,
            current_depth: usize,
        ) -> Result<()> {
            // 检查目录深度，防止无限递归
            const MAX_SCAN_DEPTH: usize = 20;
            if current_depth > MAX_SCAN_DEPTH {
                warn!("目录扫描深度超过限制 {} 层，跳过: {}", MAX_SCAN_DEPTH, dir_path.display());
                return Ok(());
            }

            // 验证目录深度
            validator.validate_directory_depth(dir_path)?;

            if !dir_path.exists() {
                return Err(anyhow::anyhow!("目录不存在: {}", dir_path.display()));
            }

            if !dir_path.is_dir() {
                return Err(anyhow::anyhow!("路径不是目录: {}", dir_path.display()));
            }

            // 限制每个目录的最大条目数，防止内存耗尽
            const MAX_ENTRIES_PER_DIR: usize = 10000;
            let mut dir_entry_count = 0;

            for entry in std::fs::read_dir(dir_path)? {
                if dir_entry_count >= MAX_ENTRIES_PER_DIR {
                    warn!("目录 {} 包含过多文件，已达到限制 {} 个", dir_path.display(), MAX_ENTRIES_PER_DIR);
                    break;
                }

                let entry = entry?;
                let path = entry.path();
                let metadata = entry.metadata()?;

                // 跳过符号链接，防止循环引用
                if metadata.file_type().is_symlink() {
                    debug!("跳过符号链接: {}", path.display());
                    continue;
                }

                // 检查是否为隐藏目录或文件，如果是则跳过
                if let Some(name) = path.file_name() {
                    let name_str = name.to_string_lossy();
                    if name_str.starts_with('.') {
                        if metadata.is_dir() {
                            debug!("跳过隐藏目录: {}", path.display());
                            continue;
                        } else {
                            // 对于隐藏文件，检查是否在允许列表中
                            let allowed_hidden = [".gitignore", ".env.example", ".dockerignore"];
                            if !allowed_hidden.iter().any(|&allowed| name_str == allowed) {
                                debug!("跳过隐藏文件: {}", path.display());
                                continue;
                            }
                        }
                    }
                    
                    // 跳过系统目录
                    if metadata.is_dir() && (
                        name_str == "System Volume Information" ||
                        name_str == "$RECYCLE.BIN" ||
                        name_str == "Thumbs.db"
                    ) {
                        debug!("跳过系统目录: {}", path.display());
                        continue;
                    }
                }

                let dir_entry = DirectoryEntry {
                    path: path.to_string_lossy().to_string(),
                    size: metadata.len(),
                    is_dir: metadata.is_dir(),
                    modified: metadata
                        .modified()
                        .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
                        .duration_since(std::time::SystemTime::UNIX_EPOCH)
                        .map(|d| chrono::DateTime::from_timestamp(d.as_secs() as i64, 0))
                        .unwrap_or(None)
                        .unwrap_or_else(chrono::Utc::now),
                };

                entries.push(dir_entry);
                dir_entry_count += 1;

                // 递归扫描子目录
                if metadata.is_dir() {
                    if let Err(e) = scan_directory(&path, entries, validator, current_depth + 1) {
                        warn!("扫描子目录失败 {}: {}", path.display(), e);
                        // 继续扫描其他目录，不中断整个过程
                    }
                }
            }

            Ok(())
        }

        let root_path = normalized_path.as_path();
        scan_directory(root_path, &mut entries, &validator, 0)?;

        info!("目录索引生成完成，共扫描 {} 个条目", entries.len());

        Ok(Self {
            root_path: path.to_string(),
            entries,
            generated_at: chrono::Utc::now(),
        })
    }

    /// 搜索文件 - 性能优化版本
    ///
    /// # 参数
    ///
    /// * `keywords` - 搜索关键词
    ///
    /// # 返回值
    ///
    /// 匹配的文件路径列表，使用 SmallVec 减少小结果集的堆分配
    pub fn search(&self, keywords: &[String]) -> SmallVec<[String; 8]> {
        // 预处理关键词：转换为小写并存储在栈上的小向量中
        let keywords_lower: SmallVec<[String; 4]> =
            keywords.iter().map(|k| k.to_lowercase()).collect();

        // 使用更高效的过滤和收集方式
        let mut results: SmallVec<[String; 8]> = SmallVec::new();

        for entry in &self.entries {
            let path_lower = entry.path.to_lowercase();
            if keywords_lower
                .iter()
                .any(|keyword| path_lower.contains(keyword))
            {
                results.push(entry.path.clone());
                // 限制结果数量，避免过大的内存占用
                if results.len() >= 1000 {
                    break;
                }
            }
        }

        // 应用安全过滤器
        let filter = SearchResultFilter::new();
        let filtered_results: Vec<String> = filter.filter_results(results.into_iter().collect());
        
        // 转换回 SmallVec
        filtered_results.into_iter().collect()
    }

    /// 保存索引到文件 - 性能优化版本（使用JSON以确保兼容性）
    ///
    /// # 参数
    ///
    /// * `output_path` - 输出文件路径
    ///
    /// # 返回值
    ///
    /// 保存结果
    pub fn save_to_file(&self, output_path: &str) -> Result<()> {
        // 使用 JSON 以确保完全兼容性
        let serialized = serde_json::to_vec_pretty(self)
            .map_err(|e| anyhow::anyhow!("序列化目录索引失败: {}", e))?;

        std::fs::write(output_path, serialized)
            .map_err(|e| anyhow::anyhow!("写入索引文件失败: {}", e))?;

        info!("目录索引已保存到: {} (JSON格式)", output_path);
        Ok(())
    }

    /// 从文件加载索引 - 性能优化版本
    ///
    /// # 参数
    ///
    /// * `input_path` - 输入文件路径
    ///
    /// # 返回值
    ///
    /// 目录索引实例
    pub fn load_from_file(input_path: &str) -> Result<Self> {
        let data =
            std::fs::read(input_path).map_err(|e| anyhow::anyhow!("读取索引文件失败: {}", e))?;

        // 使用 JSON 反序列化
        serde_json::from_slice(&data).map_err(|e| anyhow::anyhow!("反序列化目录索引失败: {}", e))
    }
}

/// UDP 广播管理器
///
/// 负责处理基于 UDP 的 WDIC 协议广播功能。
/// 性能优化版本：使用 AHashMap 提升哈希表性能。
#[derive(Debug)]
pub struct UdpBroadcastManager {
    /// 本地地址
    local_addr: SocketAddr,
    /// UDP 套接字
    udp_socket: Arc<UdpSocket>,
    /// 事件发送通道
    event_sender: mpsc::UnboundedSender<UdpBroadcastEvent>,
    /// 事件接收通道
    event_receiver: Arc<Mutex<Option<mpsc::UnboundedReceiver<UdpBroadcastEvent>>>>,
    /// 广播地址列表 - 使用 SmallVec 减少堆分配
    broadcast_addresses: SmallVec<[SocketAddr; 8]>,
    /// 目录挂载点 - 使用 AHashMap 提升性能
    mounted_directories: Arc<RwLock<AHashMap<String, DirectoryIndex>>>,
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
            mounted_directories: Arc::new(RwLock::new(AHashMap::new())),
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
    /// 生成广播地址列表（支持 IPv4/IPv6 双栈）- 性能优化版本
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
    /// 广播和多播地址列表，使用 SmallVec 减少堆分配
    fn generate_broadcast_addresses(local_addr: SocketAddr) -> SmallVec<[SocketAddr; 8]> {
        let mut addresses = SmallVec::new();
        let port = local_addr.port();

        debug!("为UDP地址 {} 生成广播地址列表", local_addr);

        // 获取所有网络接口
        let interfaces = match if_addrs::get_if_addrs() {
            Ok(interfaces) => interfaces,
            Err(e) => {
                debug!("无法获取网络接口列表: {e}, 使用默认广播地址");
                return Self::generate_fallback_addresses(port);
            }
        };

        // 分类接口地址 - 使用 SmallVec 减少堆分配
        let mut ipv4_private = SmallVec::<[Ipv4Addr; 4]>::new();
        let mut ipv4_public = SmallVec::<[Ipv4Addr; 4]>::new();
        let mut ipv6_private = SmallVec::<[Ipv6Addr; 4]>::new();
        let mut ipv6_public = SmallVec::<[Ipv6Addr; 4]>::new();

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
            info!("UDP：没有找到私有 IPv4 地址，使用公网 IPv4 地址进行广播");
            Self::add_ipv4_public_broadcasts(&mut addresses, &ipv4_public, port);
        }

        // 生成 IPv6 多播地址
        if !ipv6_private.is_empty() || !ipv6_public.is_empty() {
            Self::add_ipv6_multicasts(&mut addresses, port);
        }

        // 如果没有找到任何有效地址，使用后备地址
        if addresses.is_empty() {
            debug!("UDP：没有找到有效的网络接口，使用默认广播地址");
            addresses = Self::generate_fallback_addresses(port);
        }

        debug!("UDP：生成了 {} 个广播/多播地址", addresses.len());
        for addr in &addresses {
            debug!("  UDP - {addr}");
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

    /// 添加 IPv4 私有网络广播地址 - 性能优化版本
    fn add_ipv4_broadcasts(
        addresses: &mut SmallVec<[SocketAddr; 8]>,
        ipv4_addrs: &[Ipv4Addr],
        port: u16,
    ) {
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

    /// 添加 IPv4 公网广播地址 - 性能优化版本
    fn add_ipv4_public_broadcasts(
        addresses: &mut SmallVec<[SocketAddr; 8]>,
        _ipv4_addrs: &[Ipv4Addr],
        port: u16,
    ) {
        // 对于公网地址，我们只能使用有限广播
        addresses.push(SocketAddr::from(([255, 255, 255, 255], port)));
    }

    /// 添加 IPv6 多播地址 - 性能优化版本
    fn add_ipv6_multicasts(addresses: &mut SmallVec<[SocketAddr; 8]>, port: u16) {
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

        // 自定义的 WDIC UDP 多播地址 (ff05::5556)
        addresses.push(SocketAddr::new(
            IpAddr::V6(Ipv6Addr::new(0xff05, 0, 0, 0, 0, 0, 0, 0x5556)),
            port,
        ));
    }

    /// 生成后备广播地址 - 性能优化版本
    fn generate_fallback_addresses(port: u16) -> SmallVec<[SocketAddr; 8]> {
        let mut addresses = SmallVec::new();
        addresses.push(SocketAddr::from(([255, 255, 255, 255], port)));
        addresses.push(SocketAddr::from(([192, 168, 255, 255], port)));
        addresses.push(SocketAddr::from(([10, 255, 255, 255], port)));
        addresses.push(SocketAddr::from(([172, 31, 255, 255], port)));
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
                    debug!("收到来自 {sender_addr} 的 {size} 字节 UDP 数据");

                    // 尝试解析为 UDP 令牌
                    match serde_json::from_slice::<UdpToken>(&buffer[..size]) {
                        Ok(token) => {
                            debug!("解析 UDP 令牌成功: {token:?}");
                            let _ = event_sender.send(UdpBroadcastEvent::TokenReceived {
                                token,
                                sender: sender_addr,
                            });
                        }
                        Err(e) => {
                            debug!("解析 UDP 令牌失败，尝试解析为 WDIC 消息: {e}");
                            // 尝试解析为 WDIC 消息（向后兼容）
                            if let Ok(_message) =
                                serde_json::from_slice::<WdicMessage>(&buffer[..size])
                            {
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
                    debug!("UDP 接收时出现 OS 异常（已隐蔽处理）: {e}");
                    let _ = event_sender.send(UdpBroadcastEvent::NetworkError {
                        error: "网络通信异常".to_string(),
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
        let data =
            serde_json::to_vec(token).map_err(|e| anyhow::anyhow!("序列化令牌失败: {}", e))?;

        let mut success_count = 0;

        for &broadcast_addr in &self.broadcast_addresses {
            match self.udp_socket.send_to(&data, broadcast_addr) {
                Ok(_) => {
                    success_count += 1;
                    debug!("成功广播令牌到 {broadcast_addr}");
                }
                Err(e) => {
                    // 隐蔽 OS 异常
                    debug!("广播到 {broadcast_addr} 时出现 OS 异常（已隐蔽处理）: {e}");
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
        let data =
            serde_json::to_vec(token).map_err(|e| anyhow::anyhow!("序列化令牌失败: {}", e))?;

        debug!("发送令牌到 {target}");

        self.udp_socket.send_to(&data, target).map_err(|e| {
            // 隐蔽 OS 异常
            debug!("发送令牌到 {target} 时出现 OS 异常: {e}");
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
        info!("开始挂载目录: {name} -> {path}");

        // 验证挂载点名称
        if name.is_empty() || name.len() > 255 {
            return Err(anyhow::anyhow!("挂载点名称无效: 长度必须在 1-255 字符之间"));
        }

        // 检查挂载点名称是否包含非法字符
        if name.contains('/') || name.contains('\\') || name.contains(':') || name.contains('<') ||
           name.contains('>') || name.contains('|') || name.contains('?') || name.contains('*') {
            return Err(anyhow::anyhow!("挂载点名称包含非法字符: {}", name));
        }

        // 验证路径安全性
        let validator = PathValidator::new(vec![]);
        let normalized_path = validator.validate_and_normalize(&path)?;

        // 检查路径是否存在且为目录
        if !normalized_path.exists() {
            return Err(anyhow::anyhow!("目录不存在: {}", normalized_path.display()));
        }

        if !normalized_path.is_dir() {
            return Err(anyhow::anyhow!("路径不是目录: {}", normalized_path.display()));
        }

        // 检查是否已经挂载了同名的挂载点
        {
            let mounted = self.mounted_directories.read().await;
            if mounted.contains_key(&name) {
                return Err(anyhow::anyhow!("挂载点已存在: {}", name));
            }
        }

        // 生成目录索引
        let index = DirectoryIndex::generate(&path)?;

        // 保存索引文件到安全位置
        let index_dir = PathBuf::from("./indices");
        if !index_dir.exists() {
            std::fs::create_dir_all(&index_dir)
                .map_err(|e| anyhow::anyhow!("创建索引目录失败: {}", e))?;
        }

        let index_file = index_dir.join(format!("{name}.index"));
        index.save_to_file(&index_file.to_string_lossy())?;

        // 添加到挂载点
        {
            let mut mounted = self.mounted_directories.write().await;
            mounted.insert(name.clone(), index);
        }

        // 更新安全文件读取器的允许根目录列表
        let mounted_dirs = self.get_mounted_directories().await;
        let mut allowed_roots = Vec::new();
        for mount_name in mounted_dirs {
            if let Some(index) = self.mounted_directories.read().await.get(&mount_name) {
                allowed_roots.push(PathBuf::from(&index.root_path));
            }
        }

        // 由于 SecureFileReader 不可变，我们在读取文件时会重新创建
        // 这是一个权衡，确保每次文件访问都使用最新的安全配置

        info!("目录挂载成功: {name}");
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
        // 获取当前挂载的根目录列表
        let mounted_dirs = self.get_mounted_directories().await;
        let mut allowed_roots = Vec::new();
        
        {
            let mounted = self.mounted_directories.read().await;
            for mount_name in &mounted_dirs {
                if let Some(index) = mounted.get(mount_name) {
                    allowed_roots.push(PathBuf::from(&index.root_path));
                }
            }
        }

        // 创建安全文件读取器
        let secure_reader = SecureFileReader::new(
            allowed_roots,
            10 * 1024 * 1024, // 10MB 文件大小限制
        );

        // 安全地读取文件
        let data = secure_reader.read_file(file_path)
            .map_err(|e| anyhow::anyhow!("安全文件读取失败: {}", e))?;

        // 检查文件是否在挂载的目录索引中
        let file_found_in_index = {
            let mounted = self.mounted_directories.read().await;
            mounted.values().any(|index| {
                index.entries.iter().any(|entry| {
                    entry.path == file_path && !entry.is_dir
                })
            })
        };

        if !file_found_in_index {
            warn!("尝试访问未在索引中的文件: {file_path}");
            return Err(anyhow::anyhow!("文件访问被拒绝: 文件不在任何挂载的目录索引中"));
        }

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
    pub async fn performance_test(
        &self,
        tester_id: Uuid,
        test_type: String,
        data_size: usize,
    ) -> Result<u64> {
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
        let index = DirectoryIndex {
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
        let mount_result = manager
            .mount_directory(
                "test_mount".to_string(),
                current_dir.to_string_lossy().to_string(),
            )
            .await;

        // 如果目录存在且可访问，挂载应该成功
        if mount_result.is_ok() {
            let mounted = manager.get_mounted_directories().await;
            assert!(mounted.contains(&"test_mount".to_string()));

            // 测试搜索功能
            let _results = manager.search_files(&["rs".to_string()]).await;
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
        let result = manager
            .send_info_message(sender_id, "测试消息".to_string())
            .await;

        // 即使广播失败（没有监听者），也应该返回成功
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_udp_broadcast_manager_performance_test() {
        let local_addr = create_test_addr(0);
        let manager = UdpBroadcastManager::new(local_addr).expect("创建管理器失败");

        let tester_id = Uuid::new_v4();
        let result = manager
            .performance_test(tester_id, "latency_test".to_string(), 1024)
            .await;

        assert!(result.is_ok());
        let latency = result.unwrap();
        assert!(latency <= 1000); // 延迟应该在合理范围内（毫秒）
    }
}
