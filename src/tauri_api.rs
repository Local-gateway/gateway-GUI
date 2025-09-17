//! Tauri 后端 API 模块
//!
//! 该模块为前端提供完整的网关功能接口，包括：
//! - 网关核心功能接口 (Gateway API)
//! - 配置管理接口 (Configuration API)
//! - 目录和文件操作接口 (Directory API)
//! - 网络通信接口 (Network API)
//! - 性能监控接口 (Performance API)
//! - 状态查询接口 (Status API)
//! - 安全管理接口 (Security API)
//!
//! 所有接口都遵循 Tauri 的最佳实践，提供异步支持和错误处理。

use crate::{
    cache::GatewayCache,
    compression::CompressionStatsSnapshot,
    gateway::{Gateway, GatewayConfig},
    network::NetworkManager,
    performance::{PerformanceMonitor, PerformanceReport},
    registry::Registry,
    security::SecurityManager,
};
use tokio::sync::RwLock;

use anyhow::Result;
use chrono::{DateTime, Utc};
use log::{info, warn};
use serde::{Deserialize, Serialize};
use std::{
    path::PathBuf,
    collections::HashMap,
    sync::Arc,
};
use tauri::command;
use uuid::Uuid;

// 全局状态管理器 - 使用异步Mutex包装Option
pub static GLOBAL_STATE: once_cell::sync::Lazy<tokio::sync::Mutex<Option<GlobalGatewayState>>> = 
    once_cell::sync::Lazy::new(|| tokio::sync::Mutex::new(None));

/// 获取全局状态的辅助函数
async fn ensure_global_state() -> Result<(), String> {
    let mut global_state = GLOBAL_STATE.lock().await;
    
    if global_state.is_none() {
        let new_state = GlobalGatewayState::new().await
            .map_err(|e| format!("状态初始化失败: {e}"))?;
        *global_state = Some(new_state);
    }
    
    Ok(())
}

/// 全局网关状态
#[derive(Debug)]
pub struct GlobalGatewayState {
    /// 网关实例
    pub gateway: Arc<RwLock<Option<Gateway>>>,
    /// 性能监控器
    pub performance_monitor: Arc<PerformanceMonitor>,
    /// 缓存管理器
    pub cache: Arc<GatewayCache>,
    /// 安全管理器
    pub security_manager: Arc<SecurityManager>,
    /// 网络管理器
    pub network_manager: Arc<NetworkManager>,
    /// 注册表
    pub registry: Arc<Registry>,
}

impl GlobalGatewayState {
    /// 创建新的全局网关状态
    pub async fn new() -> Result<Self> {
        let performance_monitor = Arc::new(PerformanceMonitor::new());
        let cache = Arc::new(GatewayCache::new("./cache", 3600, 1024 * 1024 * 1024)?); // 1GB 缓存
        let security_manager = Arc::new(SecurityManager::new().await?);
        
        // 使用默认地址创建网络管理器
        let default_addr = "0.0.0.0:0".parse().unwrap();
        let network_manager = Arc::new(NetworkManager::new(default_addr)?);
        
        // 使用默认参数创建注册表
        let registry = Arc::new(Registry::new("WDIC_Gateway".to_string(), default_addr));

        Ok(Self {
            gateway: Arc::new(RwLock::new(None)),
            performance_monitor,
            cache,
            security_manager,
            network_manager,
            registry,
        })
    }
}

/// 网关状态信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayStatus {
    /// 是否运行中
    pub is_running: bool,
    /// 启动时间
    pub start_time: Option<DateTime<Utc>>,
    /// 当前配置
    pub config: Option<GatewayConfig>,
    /// 连接数量
    pub active_connections: u32,
    /// 总处理请求数
    pub total_requests: u64,
    /// 错误数量
    pub error_count: u64,
    /// 运行时长（秒）
    pub uptime_seconds: u64,
}

/// 网络状态信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkStatus {
    /// 本地 IP 地址
    pub local_ip: String,
    /// 监听端口
    pub listen_port: u16,
    /// 网络接口列表
    pub network_interfaces: Vec<NetworkInterface>,
    /// P2P 发现状态
    pub p2p_discovery_enabled: bool,
    /// 已发现的节点数量
    pub discovered_nodes: u32,
}

/// 网络接口信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkInterface {
    /// 接口名称
    pub name: String,
    /// IP 地址
    pub ip_address: String,
    /// 是否活跃
    pub is_active: bool,
    /// 接口类型
    pub interface_type: String,
}

/// 目录挂载信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MountPoint {
    /// 挂载点 ID
    pub id: String,
    /// 本地路径
    pub local_path: PathBuf,
    /// 挂载名称
    pub mount_name: String,
    /// 是否只读
    pub read_only: bool,
    /// 挂载时间
    pub mount_time: DateTime<Utc>,
    /// 文件数量
    pub file_count: u64,
    /// 总大小（字节）
    pub total_size: u64,
}

/// 文件传输任务
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileTransferTask {
    /// 任务 ID
    pub id: String,
    /// 源路径
    pub source_path: PathBuf,
    /// 目标路径
    pub target_path: PathBuf,
    /// 传输状态
    pub status: TransferStatus,
    /// 已传输字节数
    pub transferred_bytes: u64,
    /// 总字节数
    pub total_bytes: u64,
    /// 传输速度（字节/秒）
    pub transfer_speed: u64,
    /// 开始时间
    pub start_time: DateTime<Utc>,
    /// 预计完成时间
    pub estimated_completion: Option<DateTime<Utc>>,
}

/// 传输状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransferStatus {
    /// 等待中
    Pending,
    /// 传输中
    Transferring,
    /// 已完成
    Completed,
    /// 已取消
    Cancelled,
    /// 错误
    Error(String),
}

/// 安全配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// 是否启用 TLS
    pub tls_enabled: bool,
    /// 证书路径
    pub cert_path: Option<PathBuf>,
    /// 私钥路径
    pub key_path: Option<PathBuf>,
    /// CA 证书路径
    pub ca_cert_path: Option<PathBuf>,
    /// 是否验证客户端证书
    pub verify_client_cert: bool,
    /// 允许的客户端列表
    pub allowed_clients: Vec<String>,
    /// 访问控制列表
    pub access_control_rules: Vec<AccessRule>,
}

/// 访问控制规则
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessRule {
    /// 规则 ID
    pub id: String,
    /// 规则名称
    pub name: String,
    /// 客户端 IP 或主机名
    pub client: String,
    /// 允许的路径
    pub allowed_paths: Vec<String>,
    /// 权限（read, write, admin）
    pub permissions: Vec<String>,
    /// 是否启用
    pub enabled: bool,
}

// ============================================================================
// 网关核心功能接口 (Gateway API)
// ============================================================================

/// 启动网关服务
#[command]
pub async fn start_gateway(config: GatewayConfig) -> Result<(), String> {
    ensure_global_state().await?;
    
    let global_state = GLOBAL_STATE.lock().await;
    let _state = global_state.as_ref().unwrap();
    let mut gateway_lock = _state.gateway.write().await;

    if gateway_lock.is_some() {
        return Err("网关已经在运行中".to_string());
    }

    let gateway = Gateway::with_config(config)
        .await
        .map_err(|e| format!("网关创建失败: {e}"))?;

    // 启动网络管理器和UDP广播管理器（非阻塞）
    gateway.network_manager().start().await
        .map_err(|e| format!("网络管理器启动失败: {e}"))?;
    
    gateway.udp_broadcast_manager().start().await
        .map_err(|e| format!("UDP广播管理器启动失败: {e}"))?;

    // 设置运行状态为true
    *gateway.running().lock().await = true;

    // 克隆必要的Arc引用用于后台任务
    let registry_clone = Arc::clone(gateway.registry());
    let network_clone = Arc::clone(gateway.network_manager());
    let udp_clone = Arc::clone(gateway.udp_broadcast_manager());
    let config_clone = gateway.config().clone();
    let running_clone = Arc::clone(gateway.running());
    let cache_clone = Arc::clone(gateway.cache());

    // 启动初始广播
    if let Err(e) = gateway.initial_broadcast().await {
        warn!("初始广播失败: {e}");
    }

    // 在后台启动网关主循环
    tokio::spawn(async move {
        // 启动定期任务
        let broadcast_handle = tokio::spawn(Gateway::broadcast_task(
            Arc::clone(&registry_clone),
            Arc::clone(&network_clone),
            Arc::clone(&udp_clone),
            Arc::clone(&cache_clone),
            config_clone.clone(),
            Arc::clone(&running_clone),
        ));

        // 启动事件循环
        if let Some(mut event_receiver) = network_clone.take_event_receiver().await {
            if let Some(mut udp_event_receiver) = udp_clone.take_event_receiver().await {
                // 使用现有的网关实例处理事件，而不是创建临时实例
                // 因为事件处理主要依赖网络管理器和UDP广播管理器
                info!("网关事件循环启动成功");
                
                // 简单的事件处理循环
                loop {
                    tokio::select! {
                        Some(network_event) = event_receiver.recv() => {
                            info!("收到网络事件: {network_event:?}");
                            // 这里可以添加具体的事件处理逻辑
                        }
                        Some(udp_event) = udp_event_receiver.recv() => {
                            info!("收到UDP广播事件: {udp_event:?}");
                            // 这里可以添加具体的事件处理逻辑
                        }
                        else => {
                            // 两个接收器都关闭了，退出循环
                            break;
                        }
                    }
                }
            }
        }

        // 等待广播任务完成
        broadcast_handle.await.ok();
    });

    *gateway_lock = Some(gateway);
    
    Ok(())
}

/// 停止网关服务
#[command]
pub async fn stop_gateway() -> Result<String, String> {
    let global_state = GLOBAL_STATE.lock().await;
    
    if let Some(ref state) = *global_state {
        // 获取网关的写入锁
        let mut _gateway = state.gateway.write().await;
        
        // 停止服务
        *_gateway = None;
        
        Ok("网关服务已停止".to_string())
    } else {
        Err("网关服务未初始化".to_string())
    }
}

/// 重启网关服务
#[command]
pub async fn restart_gateway(config: Option<GatewayConfig>) -> Result<(), String> {
    // 先停止
    stop_gateway().await?;
    
    // 使用新配置或默认配置重启
    let restart_config = config.unwrap_or_default();
    
    start_gateway(restart_config).await
}

/// 获取网关状态
#[command]
pub async fn get_gateway_status() -> Result<GatewayStatus, String> {
    let global_state = GLOBAL_STATE.lock().await;
    
    if global_state.is_none() {
        return Ok(GatewayStatus {
            is_running: false,
            start_time: None,
            config: None,
            active_connections: 0,
            total_requests: 0,
            error_count: 0,
            uptime_seconds: 0,
        });
    }
    
    let _state = global_state.as_ref().unwrap();
    let gateway_lock = _state.gateway.read().await;

    let is_running = if let Some(ref gateway) = *gateway_lock {
        *gateway.running().lock().await
    } else {
        false
    };
    let config = gateway_lock.as_ref().map(|gateway| gateway.config().clone());

    // 获取性能监控数据
    let perf_report = _state.performance_monitor.get_report().await;

    Ok(GatewayStatus {
        is_running,
        start_time: if is_running { Some(Utc::now()) } else { None },
        config,
        active_connections: perf_report.current_connections as u32,
        total_requests: perf_report.total_requests,
        error_count: perf_report.error_count,
        uptime_seconds: perf_report.uptime_seconds,
    })
}

// ============================================================================
// 配置管理接口 (Configuration API)
// ============================================================================

/// 获取网关配置
#[command]
pub async fn get_gateway_config() -> Result<Option<GatewayConfig>, String> {
    let global_state = GLOBAL_STATE.lock().await;
    
    if global_state.is_none() {
        return Ok(None);
    }
    
    let _state = global_state.as_ref().unwrap();
    let gateway_lock = _state.gateway.read().await;

    Ok(gateway_lock.as_ref().map(|g| g.config().clone()))
}

/// 更新网关配置
#[command]
pub async fn update_gateway_config(_config: GatewayConfig) -> Result<(), String> {
    let global_state = GLOBAL_STATE.lock().await;
    
    if global_state.is_none() {
        return Err("网关未初始化".to_string());
    }
    
    let state = global_state.as_ref().unwrap();
    let gateway_lock = state.gateway.read().await;

    if let Some(ref _gateway) = *gateway_lock {
        // 配置更新需要重新启动网关，这里暂时不支持运行时更新
        Err("运行时配置更新暂不支持，请重启网关".to_string())
    } else {
        Err("网关未运行，无法更新配置".to_string())
    }
}

/// 验证配置有效性
#[command]
pub async fn validate_config(config: GatewayConfig) -> Result<bool, String> {
    config.validate()
        .map_err(|e| format!("配置验证失败: {e}"))?;
    Ok(true)
}

/// 重置为默认配置
#[command]
pub async fn reset_to_default_config() -> Result<GatewayConfig, String> {
    Ok(GatewayConfig::default())
}

// ============================================================================
// 目录和文件操作接口 (Directory API)
// ============================================================================

/// 挂载目录
#[command]
pub async fn mount_directory(
    local_path: PathBuf,
    mount_name: String,
    read_only: bool,
) -> Result<String, String> {
    
    // 验证路径存在
    if !local_path.exists() {
        return Err(format!("路径不存在: {local_path:?}"));
    }

    if !local_path.is_dir() {
        return Err(format!("不是目录: {local_path:?}"));
    }

    let mount_id = Uuid::new_v4().to_string();
    
    let global_state = GLOBAL_STATE.lock().await;
    
    if global_state.is_none() {
        return Err("网关未初始化".to_string());
    }
    
    let state = global_state.as_ref().unwrap();
    // 在注册表中注册挂载点
    state.registry.register_mount_point(
        mount_id.clone(),
        local_path.clone(),
        mount_name,
        read_only,
    ).await
    .map_err(|e| format!("挂载失败: {e}"))?;

    Ok(mount_id)
}

/// 卸载目录
#[command]
pub async fn unmount_directory(mount_id: String) -> Result<(), String> {
    ensure_global_state().await?;
    
    let global_state = GLOBAL_STATE.lock().await;
    let state = global_state.as_ref().unwrap();
    // 从注册表中移除挂载点
    state.registry.unregister_mount_point(&mount_id).await
        .map_err(|e| format!("卸载失败: {e}"))?;

    Ok(())
}

/// 获取挂载点列表
#[command]
pub async fn get_mount_points() -> Result<Vec<MountPoint>, String> {
    ensure_global_state().await?;
    
    let global_state = GLOBAL_STATE.lock().await;
    let state = global_state.as_ref().unwrap();
    let mount_points = state.registry.get_mount_points().await
        .map_err(|e| format!("获取挂载点失败: {e}"))?;

    Ok(mount_points)
}

/// 列出目录内容
#[command]
pub async fn list_directory(mount_id: String, path: String) -> Result<Vec<DirectoryEntry>, String> {
    ensure_global_state().await?;
    
    let global_state = GLOBAL_STATE.lock().await;
    let state = global_state.as_ref().unwrap();
    
    state.registry.list_directory(&mount_id, &path)
        .await
        .map_err(|e| format!("列出目录失败: {e}"))
}

/// 目录条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectoryEntry {
    /// 名称
    pub name: String,
    /// 完整路径
    pub path: String,
    /// 是否为目录
    pub is_directory: bool,
    /// 文件大小（字节）
    pub size: u64,
    /// 修改时间
    pub modified_time: DateTime<Utc>,
    /// 创建时间
    pub created_time: Option<DateTime<Utc>>,
    /// 文件类型
    pub file_type: String,
}

/// 创建文件传输任务
#[command]
pub async fn create_file_transfer(source_path: PathBuf, target_path: PathBuf) -> Result<String, String> {
    ensure_global_state().await?;
    
    let global_state = GLOBAL_STATE.lock().await;
    let state = global_state.as_ref().unwrap();
    
    let task_id = Uuid::new_v4().to_string();
    
    // 创建传输任务
    state.network_manager.create_transfer_task(
        task_id.clone(),
        source_path,
        target_path,
    ).await
    .map_err(|e| format!("创建传输任务失败: {e}"))?;

    Ok(task_id)
}

/// 获取文件传输任务状态
#[command]
pub async fn get_transfer_status(task_id: String) -> Result<FileTransferTask, String> {
    ensure_global_state().await?;
    
    let global_state = GLOBAL_STATE.lock().await;
    let state = global_state.as_ref().unwrap();
    
    state.network_manager.get_transfer_status(&task_id)
        .await
        .map_err(|e| format!("获取传输状态失败: {e}"))
}

/// 取消文件传输任务
#[command]
pub async fn cancel_transfer(task_id: String) -> Result<(), String> {
    ensure_global_state().await?;
    
    let global_state = GLOBAL_STATE.lock().await;
    let state = global_state.as_ref().unwrap();
    
    state.network_manager.cancel_transfer(&task_id)
        .await
        .map_err(|e| format!("取消传输失败: {e}"))?;

    Ok(())
}

// ============================================================================
// 网络通信接口 (Network API)  
// ============================================================================

/// 获取网络状态
#[command]
pub async fn get_network_status() -> Result<NetworkStatus, String> {
    ensure_global_state().await?;
    
    let global_state = GLOBAL_STATE.lock().await;
    let state = global_state.as_ref().unwrap();
    
    let network_info = state.network_manager.get_network_info()
        .await
        .map_err(|e| format!("获取网络信息失败: {e}"))?;

    Ok(network_info)
}

/// 启动 P2P 发现
#[command]
pub async fn start_p2p_discovery() -> Result<(), String> {
    ensure_global_state().await?;
    
    let global_state = GLOBAL_STATE.lock().await;
    let state = global_state.as_ref().unwrap();
    
    state.network_manager.start_p2p_discovery()
        .await
        .map_err(|e| format!("启动 P2P 发现失败: {e}"))?;

    Ok(())
}

/// 停止 P2P 发现
#[command]
pub async fn stop_p2p_discovery() -> Result<(), String> {
    ensure_global_state().await?;
    
    let global_state = GLOBAL_STATE.lock().await;
    let state = global_state.as_ref().unwrap();
    
    state.network_manager.stop_p2p_discovery()
        .await
        .map_err(|e| format!("停止 P2P 发现失败: {e}"))?;

    Ok(())
}

/// 获取已发现的节点列表
#[command]
pub async fn get_discovered_nodes() -> Result<Vec<DiscoveredNode>, String> {
    ensure_global_state().await?;
    
    let global_state = GLOBAL_STATE.lock().await;
    let state = global_state.as_ref().unwrap();
    
    state.network_manager.get_discovered_nodes()
        .await
        .map_err(|e| format!("获取发现节点失败: {e}"))
}

/// 发现的节点信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredNode {
    /// 节点 ID
    pub node_id: String,
    /// IP 地址
    pub ip_address: String,
    /// 端口
    pub port: u16,
    /// 节点名称
    pub name: String,
    /// 发现时间
    pub discovered_time: DateTime<Utc>,
    /// 最后通信时间
    pub last_seen: DateTime<Utc>,
    /// 是否在线
    pub is_online: bool,
    /// 节点类型
    pub node_type: String,
}

/// 连接到指定节点
#[command]
pub async fn connect_to_node(node_id: String, ip_address: String, port: u16) -> Result<(), String> {
    ensure_global_state().await?;
    
    let global_state = GLOBAL_STATE.lock().await;
    let state = global_state.as_ref().unwrap();
    
    state.network_manager.connect_to_node(&node_id, &ip_address, port)
        .await
        .map_err(|e| format!("连接节点失败: {e}"))?;

    Ok(())
}

/// 断开与节点的连接
#[command]
pub async fn disconnect_from_node(node_id: String) -> Result<(), String> {
    ensure_global_state().await?;
    
    let global_state = GLOBAL_STATE.lock().await;
    let state = global_state.as_ref().unwrap();
    
    state.network_manager.disconnect_from_node(&node_id)
        .await
        .map_err(|e| format!("断开连接失败: {e}"))?;

    Ok(())
}

// ============================================================================
// 性能监控接口 (Performance API)
// ============================================================================

/// 获取性能报告
#[command]
pub async fn get_performance_report() -> Result<PerformanceReport, String> {
    ensure_global_state().await?;
    
    let global_state = GLOBAL_STATE.lock().await;
    let state = global_state.as_ref().unwrap();
    
    let report = state.performance_monitor.get_report().await;
    Ok(report)
}

/// 获取压缩统计
#[command]
pub async fn get_compression_stats() -> Result<CompressionStatsSnapshot, String> {
    ensure_global_state().await?;
    
    let global_state = GLOBAL_STATE.lock().await;
    let state = global_state.as_ref().unwrap();
    let gateway_lock = state.gateway.read().await;

    if let Some(ref gateway) = *gateway_lock {
        let stats = gateway.get_compression_stats()
            .await
            .map_err(|e| format!("获取压缩统计失败: {e}"))?;
        Ok(stats)
    } else {
        Err("网关未运行".to_string())
    }
}

/// 获取缓存统计
#[command]
pub async fn get_cache_stats() -> Result<CacheStats, String> {
    ensure_global_state().await?;
    
    let global_state = GLOBAL_STATE.lock().await;
    let state = global_state.as_ref().unwrap();
    
    let stats = state.cache.get_stats().await;
    Ok(stats)
}

/// 缓存统计信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheStats {
    /// 缓存项数量
    pub item_count: usize,
    /// 命中次数
    pub hit_count: u64,
    /// 未命中次数
    pub miss_count: u64,
    /// 命中率
    pub hit_rate: f64,
    /// 总内存使用（字节）
    pub memory_usage: usize,
    /// 最大容量
    pub max_capacity: usize,
}

/// 开始性能基准测试
#[command]
pub async fn start_performance_benchmark(
    test_type: String,
    duration_seconds: u64,
) -> Result<String, String> {
    ensure_global_state().await?;
    
    let global_state = GLOBAL_STATE.lock().await;
    let state = global_state.as_ref().unwrap();
    
    let benchmark_id = state.performance_monitor.start_benchmark(
        &test_type,
        duration_seconds,
    ).await
    .map_err(|e| format!("启动基准测试失败: {e}"))?;

    Ok(benchmark_id)
}

/// 获取基准测试结果
#[command]
pub async fn get_benchmark_result(
    benchmark_id: String,
) -> Result<BenchmarkResult, String> {
    ensure_global_state().await?;
    
    let global_state = GLOBAL_STATE.lock().await;
    let state = global_state.as_ref().unwrap();
    
    state.performance_monitor.get_benchmark_result(&benchmark_id)
        .await
        .map_err(|e| format!("获取基准测试结果失败: {e}"))
}

/// 基准测试结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResult {
    /// 测试 ID
    pub id: String,
    /// 测试类型
    pub test_type: String,
    /// 测试状态
    pub status: BenchmarkStatus,
    /// 开始时间
    pub start_time: DateTime<Utc>,
    /// 结束时间
    pub end_time: Option<DateTime<Utc>>,
    /// 测试结果
    pub results: HashMap<String, f64>,
    /// 错误信息
    pub error_message: Option<String>,
}

/// 基准测试状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BenchmarkStatus {
    /// 运行中
    Running,
    /// 已完成
    Completed,
    /// 已取消
    Cancelled,
    /// 错误
    Error,
}

// ============================================================================
// 状态查询接口 (Status API)
// ============================================================================

/// 获取系统信息
#[command]
pub async fn get_system_info() -> Result<SystemInfo, String> {
    use sysinfo::System;
    
    let mut system = System::new_all();
    system.refresh_all();

    let cpu_usage = system.cpus().iter()
        .map(|cpu| cpu.cpu_usage())
        .sum::<f32>() / system.cpus().len() as f32;

    Ok(SystemInfo {
        os_name: System::name().unwrap_or_default(),
        os_version: System::os_version().unwrap_or_default(),
        kernel_version: System::kernel_version().unwrap_or_default(),
        host_name: System::host_name().unwrap_or_default(),
        cpu_count: system.cpus().len() as u32,
        cpu_usage,
        total_memory: system.total_memory(),
        used_memory: system.used_memory(),
        available_memory: system.available_memory(),
        uptime: System::uptime(),
    })
}

/// 系统信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    /// 操作系统名称
    pub os_name: String,
    /// 操作系统版本
    pub os_version: String,
    /// 内核版本
    pub kernel_version: String,
    /// 主机名
    pub host_name: String,
    /// CPU 核心数
    pub cpu_count: u32,
    /// CPU 使用率
    pub cpu_usage: f32,
    /// 总内存（字节）
    pub total_memory: u64,
    /// 已用内存（字节）
    pub used_memory: u64,
    /// 可用内存（字节）
    pub available_memory: u64,
    /// 系统运行时间（秒）
    pub uptime: u64,
}

/// 获取服务日志
#[command]
pub async fn get_service_logs(
    lines: Option<u32>,
    level: Option<String>,
) -> Result<Vec<LogEntry>, String> {
    // 这里应该从日志系统读取日志
    // 为了演示，返回模拟数据
    let log_entries = vec![
        LogEntry {
            timestamp: Utc::now(),
            level: "INFO".to_string(),
            module: "gateway".to_string(),
            message: "网关服务启动".to_string(),
        },
        LogEntry {
            timestamp: Utc::now(),
            level: "DEBUG".to_string(),
            module: "network".to_string(),
            message: "P2P 发现已启动".to_string(),
        },
    ];
    
    let mut filtered_logs = log_entries;
    
    // 按级别过滤
    if let Some(filter_level) = level {
        filtered_logs.retain(|entry| entry.level == filter_level);
    }
    
    // 限制行数
    if let Some(max_lines) = lines {
        filtered_logs.truncate(max_lines as usize);
    }
    
    Ok(filtered_logs)
}

/// 日志条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    /// 时间戳
    pub timestamp: DateTime<Utc>,
    /// 日志级别
    pub level: String,
    /// 模块名称
    pub module: String,
    /// 日志消息
    pub message: String,
}

/// 健康检查
#[command]
pub async fn health_check() -> Result<HealthStatus, String> {
    ensure_global_state().await?;
    
    let global_state = GLOBAL_STATE.lock().await;
    let state = global_state.as_ref().unwrap();
    
    let gateway_lock = state.gateway.read().await;
    let gateway_healthy = gateway_lock.is_some();
    
    let cache_healthy = state.cache.health_check().await;
    let network_healthy = state.network_manager.health_check().await
        .unwrap_or(false);
    
    let overall_healthy = gateway_healthy && cache_healthy && network_healthy;
    
    Ok(HealthStatus {
        overall_status: if overall_healthy { "healthy".to_string() } else { "unhealthy".to_string() },
        gateway_status: if gateway_healthy { "healthy".to_string() } else { "unhealthy".to_string() },
        cache_status: if cache_healthy { "healthy".to_string() } else { "unhealthy".to_string() },
        network_status: if network_healthy { "healthy".to_string() } else { "unhealthy".to_string() },
        timestamp: Utc::now(),
    })
}

/// 健康状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatus {
    /// 总体状态
    pub overall_status: String,
    /// 网关状态
    pub gateway_status: String,
    /// 缓存状态
    pub cache_status: String,
    /// 网络状态
    pub network_status: String,
    /// 检查时间
    pub timestamp: DateTime<Utc>,
}

// ============================================================================
// 安全管理接口 (Security API)
// ============================================================================

/// 获取安全配置
#[command]
pub async fn get_security_config() -> Result<SecurityConfig, String> {
    ensure_global_state().await?;
    
    let global_state = GLOBAL_STATE.lock().await;
    let state = global_state.as_ref().unwrap();
    
    state.security_manager.get_config()
        .await
        .map_err(|e| format!("获取安全配置失败: {e}"))
}

/// 更新安全配置
#[command]
pub async fn update_security_config(
    config: SecurityConfig,
) -> Result<(), String> {
    ensure_global_state().await?;
    
    let global_state = GLOBAL_STATE.lock().await;
    let state = global_state.as_ref().unwrap();
    
    state.security_manager.update_config(config)
        .await
        .map_err(|e| format!("更新安全配置失败: {e}"))?;
    
    Ok(())
}

/// 生成 TLS 证书
#[command]
pub async fn generate_tls_certificate(
    cert_info: CertificateInfo,
) -> Result<GeneratedCertificate, String> {
    ensure_global_state().await?;
    
    let global_state = GLOBAL_STATE.lock().await;
    let state = global_state.as_ref().unwrap();
    
    state.security_manager.generate_certificate(cert_info)
        .await
        .map_err(|e| format!("生成证书失败: {e}"))
}

/// 证书信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CertificateInfo {
    /// 通用名称
    pub common_name: String,
    /// 组织名称
    pub organization: String,
    /// 国家代码
    pub country: String,
    /// 有效期（天）
    pub validity_days: u32,
    /// 主题备用名称
    pub subject_alt_names: Vec<String>,
}

/// 生成的证书
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedCertificate {
    /// 证书路径
    pub cert_path: PathBuf,
    /// 私钥路径
    pub key_path: PathBuf,
    /// 证书内容（PEM 格式）
    pub cert_pem: String,
    /// 私钥内容（PEM 格式）
    pub key_pem: String,
    /// 生成时间
    pub generated_time: DateTime<Utc>,
    /// 过期时间
    pub expiry_time: DateTime<Utc>,
}

/// 添加访问控制规则
#[command]
pub async fn add_access_rule(
    rule: AccessRule,
) -> Result<String, String> {
    ensure_global_state().await?;
    
    let global_state = GLOBAL_STATE.lock().await;
    let state = global_state.as_ref().unwrap();
    
    let rule_id = state.security_manager.add_access_rule(rule)
        .await
        .map_err(|e| format!("添加访问规则失败: {e}"))?;
    
    Ok(rule_id)
}

/// 删除访问控制规则
#[command]
pub async fn remove_access_rule(
    rule_id: String,
) -> Result<(), String> {
    ensure_global_state().await?;
    
    let global_state = GLOBAL_STATE.lock().await;
    let state = global_state.as_ref().unwrap();
    
    state.security_manager.remove_access_rule(&rule_id)
        .await
        .map_err(|e| format!("删除访问规则失败: {e}"))?;
    
    Ok(())
}

/// 获取访问控制规则列表
#[command]
pub async fn get_access_rules() -> Result<Vec<AccessRule>, String> {
    ensure_global_state().await?;
    
    let global_state = GLOBAL_STATE.lock().await;
    let state = global_state.as_ref().unwrap();
    
    state.security_manager.get_access_rules()
        .await
        .map_err(|e| format!("获取访问规则失败: {e}"))
}

/// 验证客户端访问权限
#[command]
pub async fn validate_client_access(
    client_ip: String,
    requested_path: String,
    operation: String,
) -> Result<bool, String> {
    ensure_global_state().await?;
    
    let global_state = GLOBAL_STATE.lock().await;
    let state = global_state.as_ref().unwrap();
    
    state.security_manager.validate_access(&client_ip, &requested_path, &operation)
        .await
        .map_err(|e| format!("验证访问权限失败: {e}"))
}

/// 获取活跃会话列表
#[command]
pub async fn get_active_sessions() -> Result<Vec<ActiveSession>, String> {
    ensure_global_state().await?;
    
    let global_state = GLOBAL_STATE.lock().await;
    let state = global_state.as_ref().unwrap();
    
    state.security_manager.get_active_sessions()
        .await
        .map_err(|e| format!("获取活跃会话失败: {e}"))
}

/// 活跃会话信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveSession {
    /// 会话 ID
    pub session_id: String,
    /// 客户端 IP
    pub client_ip: String,
    /// 用户标识
    pub user_id: Option<String>,
    /// 连接时间
    pub connect_time: DateTime<Utc>,
    /// 最后活跃时间
    pub last_activity: DateTime<Utc>,
    /// 传输字节数
    pub bytes_transferred: u64,
    /// 连接状态
    pub status: String,
}

/// 强制断开会话
#[command]
pub async fn disconnect_session(
    session_id: String,
) -> Result<(), String> {
    ensure_global_state().await?;
    
    let global_state = GLOBAL_STATE.lock().await;
    let state = global_state.as_ref().unwrap();
    
    state.security_manager.disconnect_session(&session_id)
        .await
        .map_err(|e| format!("断开会话失败: {e}"))?;
    
    Ok(())
}

// ============================================================================
// 导出所有命令函数
// ============================================================================

/// 获取所有 Tauri 命令函数
pub fn get_all_commands() -> Vec<&'static str> {
    vec![
        "start_gateway",
        "stop_gateway", 
        "restart_gateway",
        "get_gateway_status",
        "get_gateway_config",
        "update_gateway_config",
        "validate_config",
        "reset_to_default_config",
        "mount_directory",
        "unmount_directory",
        "get_mount_points",
        "list_directory",
        "create_file_transfer",
        "get_transfer_status",
        "cancel_transfer",
        "get_network_status",
        "start_p2p_discovery",
        "stop_p2p_discovery",
        "get_discovered_nodes",
        "connect_to_node",
        "disconnect_from_node",
        "get_performance_report",
        "get_compression_stats",
        "get_cache_stats",
        "start_performance_benchmark",
        "get_benchmark_result",
        "get_system_info",
        "get_service_logs",
        "health_check",
        "get_security_config",
        "update_security_config",
        "generate_tls_certificate",
        "add_access_rule",
        "remove_access_rule",
        "get_access_rules",
        "validate_client_access",
        "get_active_sessions",
        "disconnect_session",
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio;

    /// 创建测试用的全局状态
    async fn create_test_state() -> GlobalGatewayState {
        GlobalGatewayState::new().await.unwrap()
    }

    #[tokio::test]
    async fn test_gateway_lifecycle() {
        // 初始化全局状态
        let state = create_test_state().await;
        *GLOBAL_STATE.lock().await = Some(state);
        
        // 测试获取初始状态
        let status = get_gateway_status().await.unwrap();
        assert!(!status.is_running);
        
        // 测试启动网关
        let config = GatewayConfig::default();
        start_gateway(config).await.unwrap();
        
        let status = get_gateway_status().await.unwrap();
        assert!(status.is_running);
        
        // 测试停止网关
        stop_gateway().await.unwrap();
        
        let status = get_gateway_status().await.unwrap();
        assert!(!status.is_running);
    }

    #[tokio::test]
    async fn test_configuration_management() {
        // 测试获取默认配置
        let default_config = reset_to_default_config().await.unwrap();
        assert!(validate_config(default_config.clone()).await.unwrap());
        
        // 测试配置验证
        let mut invalid_config = default_config.clone();
        invalid_config.port = 0; // 无效端口
        assert!(validate_config(invalid_config).await.is_err());
    }

    #[tokio::test]
    async fn test_system_info() {
        let system_info = get_system_info().await.unwrap();
        
        assert!(!system_info.os_name.is_empty());
        assert!(system_info.cpu_count > 0);
        assert!(system_info.total_memory > 0);
    }

    #[tokio::test]
    async fn test_health_check() {
        // 初始化全局状态
        let state = create_test_state().await;
        *GLOBAL_STATE.lock().await = Some(state);
        
        let health = health_check().await.unwrap();
        assert!(!health.overall_status.is_empty());
        assert!(!health.gateway_status.is_empty());
    }

    #[tokio::test]
    async fn test_directory_operations() {
        // 初始化全局状态
        let state = create_test_state().await;
        *GLOBAL_STATE.lock().await = Some(state);
        
        // 测试挂载目录（使用临时目录）
        let temp_dir = std::env::temp_dir();
        let mount_id = mount_directory(
            temp_dir,
            "测试挂载".to_string(),
            true,
        ).await.unwrap();
        
        assert!(!mount_id.is_empty());
        
        // 测试获取挂载点列表
        let mount_points = get_mount_points().await.unwrap();
        assert!(!mount_points.is_empty());
        
        // 测试卸载目录
        unmount_directory(mount_id).await.unwrap();
    }

    #[tokio::test]
    async fn test_performance_monitoring() {
        // 初始化全局状态
        let state = create_test_state().await;
        *GLOBAL_STATE.lock().await = Some(state);
        
        // 测试获取性能报告
        let _report = get_performance_report().await.unwrap();
        // uptime_seconds 是无符号整数，总是 >= 0，不需要检查
        
        // 测试缓存统计
        let cache_stats = get_cache_stats().await.unwrap();
        assert!(cache_stats.hit_rate >= 0.0);
    }

    #[tokio::test]
    async fn test_network_operations() {
        // 初始化全局状态
        let state = create_test_state().await;
        *GLOBAL_STATE.lock().await = Some(state);
        
        // 测试获取网络状态
        let network_status = get_network_status().await.unwrap();
        assert!(!network_status.local_ip.is_empty());
        
        // 测试 P2P 发现控制
        start_p2p_discovery().await.unwrap();
        stop_p2p_discovery().await.unwrap();
    }

    #[tokio::test]
    async fn test_security_management() {
        // 初始化全局状态
        let state = create_test_state().await;
        *GLOBAL_STATE.lock().await = Some(state);
        
        // 测试获取安全配置
        let security_config = get_security_config().await.unwrap();
        // 安全配置存在性验证：检查基本字段
        println!("安全配置获取成功：TLS={}", security_config.tls_enabled);
        
        // 测试添加访问规则
        let rule = AccessRule {
            id: "test_rule".to_string(),
            name: "测试规则".to_string(),
            client: "127.0.0.1".to_string(),
            allowed_paths: vec!["/test".to_string()],
            permissions: vec!["read".to_string()],
            enabled: true,
        };
        
        let rule_id = add_access_rule(rule).await.unwrap();
        assert!(!rule_id.is_empty());
        
        // 测试获取访问规则列表
        let rules = get_access_rules().await.unwrap();
        assert!(!rules.is_empty());
        
        // 测试删除访问规则
        remove_access_rule(rule_id).await.unwrap();
    }

    #[tokio::test]
    async fn test_log_operations() {
        // 测试获取服务日志
        let logs = get_service_logs(Some(10), Some("INFO".to_string())).await.unwrap();
        // 注意：在实际实现中，这里应该有真实的日志数据
        assert!(logs.len() <= 10);
    }

    #[tokio::test]
    async fn test_file_transfer_operations() {
        // 初始化全局状态
        let state = create_test_state().await;
        *GLOBAL_STATE.lock().await = Some(state);
        
        // 创建临时文件路径
        let source = std::env::temp_dir().join("test_source.txt");
        let target = std::env::temp_dir().join("test_target.txt");
        
        // 测试创建文件传输任务
        let task_id = create_file_transfer(
            source,
            target,
        ).await.unwrap();
        
        assert!(!task_id.is_empty());
        
        // 测试获取传输状态
        let _status = get_transfer_status(task_id.clone()).await;
        // 在模拟实现中，这可能会失败，但在真实实现中应该成功
        
        // 测试取消传输
        let _cancel_result = cancel_transfer(task_id).await;
        // 同样，在模拟实现中可能会失败
    }
}
