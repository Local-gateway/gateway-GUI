//! 性能监控和基准测试模块
//! 
//! 提供全面的性能监控、内存使用分析和网络吞吐量测试功能。

use std::collections::HashMap;
use std::time::{Duration, Instant};
use std::sync::Arc;
use tokio::sync::{RwLock, Mutex};
use serde::{Deserialize, Serialize};
use log::info;
use anyhow::Result;

/// 性能指标收集器
#[derive(Debug)]
pub struct PerformanceMonitor {
    /// 系统信息
    system: Arc<Mutex<sysinfo::System>>,
    /// 网络指标
    network_metrics: Arc<RwLock<NetworkMetrics>>,
    /// 内存指标
    memory_metrics: Arc<RwLock<MemoryMetrics>>,
    /// 延迟指标
    latency_metrics: Arc<RwLock<LatencyMetrics>>,
    /// 连接指标
    connection_metrics: Arc<RwLock<ConnectionMetrics>>,
    /// 基准测试结果
    benchmark_results: Arc<RwLock<HashMap<String, BenchmarkResult>>>,
}

/// 网络性能指标
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NetworkMetrics {
    /// 发送的总字节数
    pub bytes_sent: u64,
    /// 接收的总字节数
    pub bytes_received: u64,
    /// 发送的总包数
    pub packets_sent: u64,
    /// 接收的总包数
    pub packets_received: u64,
    /// 发送失败次数
    pub send_errors: u64,
    /// 接收失败次数
    pub receive_errors: u64,
    /// 网络吞吐量（字节/秒）
    pub throughput_bps: f64,
    /// 上次统计时间
    pub last_update: chrono::DateTime<chrono::Utc>,
}

/// 内存性能指标
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MemoryMetrics {
    /// 当前内存使用量（字节）
    pub current_usage: u64,
    /// 峰值内存使用量（字节）
    pub peak_usage: u64,
    /// 系统总内存（字节）
    pub system_total: u64,
    /// 系统可用内存（字节）
    pub system_available: u64,
    /// 内存使用率（百分比）
    pub usage_percentage: f64,
    /// 垃圾回收次数
    pub gc_count: u64,
    /// 上次统计时间
    pub last_update: chrono::DateTime<chrono::Utc>,
}

/// 延迟性能指标
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LatencyMetrics {
    /// 平均延迟（毫秒）
    pub average_latency: f64,
    /// 最小延迟（毫秒）
    pub min_latency: f64,
    /// 最大延迟（毫秒）
    pub max_latency: f64,
    /// P50 延迟（毫秒）
    pub p50_latency: f64,
    /// P95 延迟（毫秒）
    pub p95_latency: f64,
    /// P99 延迟（毫秒）
    pub p99_latency: f64,
    /// 延迟样本数
    pub sample_count: u64,
    /// 延迟历史记录
    pub latency_history: Vec<f64>,
    /// 上次统计时间
    pub last_update: chrono::DateTime<chrono::Utc>,
}

/// 连接性能指标
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConnectionMetrics {
    /// 当前活跃连接数
    pub active_connections: u64,
    /// 总连接数
    pub total_connections: u64,
    /// 连接失败次数
    pub failed_connections: u64,
    /// 连接超时次数
    pub timeout_connections: u64,
    /// 平均连接持续时间（秒）
    pub average_connection_duration: f64,
    /// 连接成功率（百分比）
    pub connection_success_rate: f64,
    /// 上次统计时间
    pub last_update: chrono::DateTime<chrono::Utc>,
}

/// 基准测试结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResult {
    /// 测试名称
    pub name: String,
    /// 测试时长（毫秒）
    pub duration_ms: f64,
    /// 操作次数
    pub operations: u64,
    /// 每秒操作数
    pub ops_per_second: f64,
    /// 平均延迟（毫秒）
    pub average_latency: f64,
    /// 最小延迟（毫秒）
    pub min_latency: f64,
    /// 最大延迟（毫秒）
    pub max_latency: f64,
    /// 吞吐量（字节/秒）
    pub throughput_bps: f64,
    /// 内存使用量（字节）
    pub memory_usage: u64,
    /// 测试时间
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// 测试参数
    pub parameters: HashMap<String, String>,
}

/// 性能测试套件
#[derive(Debug, Clone)]
pub struct PerformanceTestSuite {
    /// 并发数
    pub concurrency: usize,
    /// 测试持续时间（秒）
    pub duration_seconds: u64,
    /// 数据包大小（字节）
    pub packet_size: usize,
    /// 测试间隔（毫秒）
    pub test_interval_ms: u64,
}

impl Default for PerformanceTestSuite {
    fn default() -> Self {
        Self {
            concurrency: 10,
            duration_seconds: 30,
            packet_size: 1024,
            test_interval_ms: 10,
        }
    }
}

impl Default for PerformanceMonitor {
    fn default() -> Self {
        Self::new()
    }
}

impl PerformanceMonitor {
    /// 创建新的性能监控器
    pub fn new() -> Self {
        let mut system = sysinfo::System::new_all();
        system.refresh_all();

        Self {
            system: Arc::new(Mutex::new(system)),
            network_metrics: Arc::new(RwLock::new(NetworkMetrics::default())),
            memory_metrics: Arc::new(RwLock::new(MemoryMetrics::default())),
            latency_metrics: Arc::new(RwLock::new(LatencyMetrics::default())),
            connection_metrics: Arc::new(RwLock::new(ConnectionMetrics::default())),
            benchmark_results: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 记录网络发送
    pub async fn record_network_send(&self, bytes: u64) {
        let mut metrics = self.network_metrics.write().await;
        metrics.bytes_sent += bytes;
        metrics.packets_sent += 1;
        metrics.last_update = chrono::Utc::now();
    }

    /// 记录网络接收
    pub async fn record_network_receive(&self, bytes: u64) {
        let mut metrics = self.network_metrics.write().await;
        metrics.bytes_received += bytes;
        metrics.packets_received += 1;
        metrics.last_update = chrono::Utc::now();
    }

    /// 记录网络错误
    pub async fn record_network_error(&self, is_send: bool) {
        let mut metrics = self.network_metrics.write().await;
        if is_send {
            metrics.send_errors += 1;
        } else {
            metrics.receive_errors += 1;
        }
        metrics.last_update = chrono::Utc::now();
    }

    /// 记录延迟
    pub async fn record_latency(&self, latency_ms: f64) {
        let mut metrics = self.latency_metrics.write().await;
        
        // 更新延迟历史（保持最近1000个样本）
        metrics.latency_history.push(latency_ms);
        if metrics.latency_history.len() > 1000 {
            metrics.latency_history.remove(0);
        }

        metrics.sample_count += 1;
        
        // 计算统计信息
        let mut sorted_history = metrics.latency_history.clone();
        sorted_history.sort_by(|a, b| a.partial_cmp(b).unwrap());
        
        if !sorted_history.is_empty() {
            metrics.min_latency = sorted_history[0];
            metrics.max_latency = sorted_history[sorted_history.len() - 1];
            metrics.average_latency = sorted_history.iter().sum::<f64>() / sorted_history.len() as f64;
            
            // 计算百分位数
            let len = sorted_history.len();
            metrics.p50_latency = sorted_history[len * 50 / 100];
            metrics.p95_latency = sorted_history[len * 95 / 100];
            metrics.p99_latency = sorted_history[len * 99 / 100];
        }
        
        metrics.last_update = chrono::Utc::now();
    }

    /// 记录连接事件
    pub async fn record_connection_event(&self, event_type: ConnectionEvent, duration_seconds: Option<f64>) {
        let mut metrics = self.connection_metrics.write().await;
        
        match event_type {
            ConnectionEvent::Connected => {
                metrics.active_connections += 1;
                metrics.total_connections += 1;
            }
            ConnectionEvent::Disconnected => {
                if metrics.active_connections > 0 {
                    metrics.active_connections -= 1;
                }
                if let Some(duration) = duration_seconds {
                    // 更新平均连接持续时间
                    let current_avg = metrics.average_connection_duration;
                    let total_closed = metrics.total_connections - metrics.active_connections;
                    if total_closed > 0 {
                        metrics.average_connection_duration = 
                            (current_avg * (total_closed - 1) as f64 + duration) / total_closed as f64;
                    }
                }
            }
            ConnectionEvent::Failed => {
                metrics.failed_connections += 1;
            }
            ConnectionEvent::Timeout => {
                metrics.timeout_connections += 1;
                if metrics.active_connections > 0 {
                    metrics.active_connections -= 1;
                }
            }
        }
        
        // 计算连接成功率
        let total_attempts = metrics.total_connections + metrics.failed_connections;
        if total_attempts > 0 {
            metrics.connection_success_rate = 
                (metrics.total_connections as f64 / total_attempts as f64) * 100.0;
        }
        
        metrics.last_update = chrono::Utc::now();
    }

    /// 更新系统资源指标
    pub async fn update_system_metrics(&self) -> Result<()> {
        let mut system = self.system.lock().await;
        system.refresh_all();

        // 更新内存指标
        {
            let mut memory_metrics = self.memory_metrics.write().await;
            
            let process = system.processes().values().find(|p| p.pid().as_u32() == std::process::id());
            if let Some(process) = process {
                memory_metrics.current_usage = process.memory() * 1024; // KB to bytes
                if memory_metrics.current_usage > memory_metrics.peak_usage {
                    memory_metrics.peak_usage = memory_metrics.current_usage;
                }
            }
            
            memory_metrics.system_total = system.total_memory() * 1024; // KB to bytes
            memory_metrics.system_available = system.available_memory() * 1024; // KB to bytes
            
            if memory_metrics.system_total > 0 {
                memory_metrics.usage_percentage = 
                    (memory_metrics.current_usage as f64 / memory_metrics.system_total as f64) * 100.0;
            }
            
            memory_metrics.last_update = chrono::Utc::now();
        }

        Ok(())
    }

    /// 运行吞吐量基准测试
    pub async fn run_throughput_benchmark(
        &self,
        test_name: &str,
        test_suite: &PerformanceTestSuite,
    ) -> Result<BenchmarkResult> {
        info!("开始吞吐量基准测试: {}", test_name);
        
        let start_time = Instant::now();
        let mut operations = 0u64;
        let mut total_bytes = 0u64;
        let mut latencies = Vec::new();
        
        let mut parameters = HashMap::new();
        parameters.insert("concurrency".to_string(), test_suite.concurrency.to_string());
        parameters.insert("duration_seconds".to_string(), test_suite.duration_seconds.to_string());
        parameters.insert("packet_size".to_string(), test_suite.packet_size.to_string());
        
        // 记录开始时的内存使用
        self.update_system_metrics().await?;
        let start_memory = self.memory_metrics.read().await.current_usage;
        
        // 运行测试
        let test_end = start_time + Duration::from_secs(test_suite.duration_seconds);
        while Instant::now() < test_end {
            let op_start = Instant::now();
            
            // 模拟网络操作
            tokio::time::sleep(Duration::from_millis(test_suite.test_interval_ms)).await;
            
            let op_duration = op_start.elapsed();
            latencies.push(op_duration.as_secs_f64() * 1000.0); // 转换为毫秒
            
            operations += 1;
            total_bytes += test_suite.packet_size as u64;
            
            // 更新网络指标
            self.record_network_send(test_suite.packet_size as u64).await;
        }
        
        let total_duration = start_time.elapsed();
        
        // 记录结束时的内存使用
        self.update_system_metrics().await?;
        let end_memory = self.memory_metrics.read().await.current_usage;
        
        // 计算统计信息
        latencies.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let avg_latency = if !latencies.is_empty() {
            latencies.iter().sum::<f64>() / latencies.len() as f64
        } else {
            0.0
        };
        
        let min_latency = latencies.first().copied().unwrap_or(0.0);
        let max_latency = latencies.last().copied().unwrap_or(0.0);
        
        let ops_per_second = operations as f64 / total_duration.as_secs_f64();
        let throughput_bps = total_bytes as f64 / total_duration.as_secs_f64();
        
        let result = BenchmarkResult {
            name: test_name.to_string(),
            duration_ms: total_duration.as_secs_f64() * 1000.0,
            operations,
            ops_per_second,
            average_latency: avg_latency,
            min_latency,
            max_latency,
            throughput_bps,
            memory_usage: end_memory.saturating_sub(start_memory),
            timestamp: chrono::Utc::now(),
            parameters,
        };
        
        // 保存结果
        self.benchmark_results.write().await.insert(test_name.to_string(), result.clone());
        
        info!("吞吐量基准测试完成: {} - {:.2} ops/s, {:.2} MB/s", 
              test_name, ops_per_second, throughput_bps / 1024.0 / 1024.0);
        
        Ok(result)
    }

    /// 运行延迟基准测试
    pub async fn run_latency_benchmark(
        &self,
        test_name: &str,
        iterations: usize,
    ) -> Result<BenchmarkResult> {
        info!("开始延迟基准测试: {} ({} 次迭代)", test_name, iterations);
        
        let start_time = Instant::now();
        let mut latencies = Vec::with_capacity(iterations);
        
        let mut parameters = HashMap::new();
        parameters.insert("iterations".to_string(), iterations.to_string());
        
        // 记录开始时的内存使用
        self.update_system_metrics().await?;
        let start_memory = self.memory_metrics.read().await.current_usage;
        
        for _ in 0..iterations {
            let op_start = Instant::now();
            
            // 模拟操作
            tokio::time::sleep(Duration::from_micros(100)).await;
            
            let latency = op_start.elapsed().as_secs_f64() * 1000.0; // 转换为毫秒
            latencies.push(latency);
            
            // 记录延迟
            self.record_latency(latency).await;
        }
        
        let total_duration = start_time.elapsed();
        
        // 记录结束时的内存使用
        self.update_system_metrics().await?;
        let end_memory = self.memory_metrics.read().await.current_usage;
        
        // 计算统计信息
        latencies.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let avg_latency = latencies.iter().sum::<f64>() / latencies.len() as f64;
        let min_latency = latencies[0];
        let max_latency = latencies[latencies.len() - 1];
        
        let ops_per_second = iterations as f64 / total_duration.as_secs_f64();
        
        let result = BenchmarkResult {
            name: test_name.to_string(),
            duration_ms: total_duration.as_secs_f64() * 1000.0,
            operations: iterations as u64,
            ops_per_second,
            average_latency: avg_latency,
            min_latency,
            max_latency,
            throughput_bps: 0.0, // 延迟测试不关注吞吐量
            memory_usage: end_memory.saturating_sub(start_memory),
            timestamp: chrono::Utc::now(),
            parameters,
        };
        
        // 保存结果
        self.benchmark_results.write().await.insert(test_name.to_string(), result.clone());
        
        info!("延迟基准测试完成: {} - 平均延迟: {:.3}ms, P95: {:.3}ms", 
              test_name, avg_latency, latencies[latencies.len() * 95 / 100]);
        
        Ok(result)
    }

    /// 获取内存指标
    pub async fn get_memory_metrics(&self) -> MemoryMetrics {
        self.memory_metrics.read().await.clone()
    }

    /// 获取网络指标
    pub async fn get_network_metrics(&self) -> NetworkMetrics {
        self.network_metrics.read().await.clone()
    }

    /// 获取延迟指标
    pub async fn get_latency_metrics(&self) -> LatencyMetrics {
        self.latency_metrics.read().await.clone()
    }

    /// 获取连接指标
    pub async fn get_connection_metrics(&self) -> ConnectionMetrics {
        self.connection_metrics.read().await.clone()
    }

    /// 获取所有性能指标
    pub async fn get_all_metrics(&self) -> PerformanceReport {
        let network = self.network_metrics.read().await.clone();
        let memory = self.memory_metrics.read().await.clone();
        let latency = self.latency_metrics.read().await.clone();
        let connection = self.connection_metrics.read().await.clone();
        let benchmarks = self.benchmark_results.read().await.clone();
        
        PerformanceReport {
            network,
            memory,
            latency,
            connection,
            benchmarks,
            generated_at: chrono::Utc::now(),
        }
    }

    /// 生成性能报告
    pub async fn generate_report(&self) -> String {
        let report = self.get_all_metrics().await;
        
        let mut output = String::new();
        output.push_str("=== 性能监控报告 ===\n");
        output.push_str(&format!("生成时间: {}\n\n", report.generated_at.format("%Y-%m-%d %H:%M:%S UTC")));
        
        // 网络性能
        output.push_str("## 网络性能\n");
        output.push_str(&format!("发送字节数: {} ({:.2} MB)\n", 
                                report.network.bytes_sent, 
                                report.network.bytes_sent as f64 / 1024.0 / 1024.0));
        output.push_str(&format!("接收字节数: {} ({:.2} MB)\n", 
                                report.network.bytes_received, 
                                report.network.bytes_received as f64 / 1024.0 / 1024.0));
        output.push_str(&format!("发送包数: {}\n", report.network.packets_sent));
        output.push_str(&format!("接收包数: {}\n", report.network.packets_received));
        output.push_str(&format!("发送错误: {}\n", report.network.send_errors));
        output.push_str(&format!("接收错误: {}\n", report.network.receive_errors));
        output.push_str(&format!("网络吞吐量: {:.2} MB/s\n\n", report.network.throughput_bps / 1024.0 / 1024.0));
        
        // 内存性能
        output.push_str("## 内存性能\n");
        output.push_str(&format!("当前使用: {:.2} MB\n", report.memory.current_usage as f64 / 1024.0 / 1024.0));
        output.push_str(&format!("峰值使用: {:.2} MB\n", report.memory.peak_usage as f64 / 1024.0 / 1024.0));
        output.push_str(&format!("系统总内存: {:.2} GB\n", report.memory.system_total as f64 / 1024.0 / 1024.0 / 1024.0));
        output.push_str(&format!("系统可用内存: {:.2} GB\n", report.memory.system_available as f64 / 1024.0 / 1024.0 / 1024.0));
        output.push_str(&format!("使用率: {:.2}%\n\n", report.memory.usage_percentage));
        
        // 延迟性能
        output.push_str("## 延迟性能\n");
        output.push_str(&format!("平均延迟: {:.3} ms\n", report.latency.average_latency));
        output.push_str(&format!("最小延迟: {:.3} ms\n", report.latency.min_latency));
        output.push_str(&format!("最大延迟: {:.3} ms\n", report.latency.max_latency));
        output.push_str(&format!("P50 延迟: {:.3} ms\n", report.latency.p50_latency));
        output.push_str(&format!("P95 延迟: {:.3} ms\n", report.latency.p95_latency));
        output.push_str(&format!("P99 延迟: {:.3} ms\n", report.latency.p99_latency));
        output.push_str(&format!("样本数: {}\n\n", report.latency.sample_count));
        
        // 连接性能
        output.push_str("## 连接性能\n");
        output.push_str(&format!("活跃连接: {}\n", report.connection.active_connections));
        output.push_str(&format!("总连接数: {}\n", report.connection.total_connections));
        output.push_str(&format!("失败连接: {}\n", report.connection.failed_connections));
        output.push_str(&format!("超时连接: {}\n", report.connection.timeout_connections));
        output.push_str(&format!("平均连接时长: {:.2} 秒\n", report.connection.average_connection_duration));
        output.push_str(&format!("连接成功率: {:.2}%\n\n", report.connection.connection_success_rate));
        
        // 基准测试结果
        if !report.benchmarks.is_empty() {
            output.push_str("## 基准测试结果\n");
            for (name, result) in &report.benchmarks {
                output.push_str(&format!("### {}\n", name));
                output.push_str(&format!("持续时间: {:.2} ms\n", result.duration_ms));
                output.push_str(&format!("操作次数: {}\n", result.operations));
                output.push_str(&format!("每秒操作数: {:.2}\n", result.ops_per_second));
                output.push_str(&format!("平均延迟: {:.3} ms\n", result.average_latency));
                output.push_str(&format!("吞吐量: {:.2} MB/s\n", result.throughput_bps / 1024.0 / 1024.0));
                output.push_str(&format!("内存使用: {:.2} MB\n", result.memory_usage as f64 / 1024.0 / 1024.0));
                output.push_str(&format!("测试时间: {}\n\n", result.timestamp.format("%Y-%m-%d %H:%M:%S UTC")));
            }
        }
        
        output
    }
}

/// 连接事件类型
#[derive(Debug, Clone)]
pub enum ConnectionEvent {
    /// 连接成功
    Connected,
    /// 连接断开
    Disconnected,
    /// 连接失败
    Failed,
    /// 连接超时
    Timeout,
}

/// 完整的性能报告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceReport {
    /// 网络指标
    pub network: NetworkMetrics,
    /// 内存指标
    pub memory: MemoryMetrics,
    /// 延迟指标
    pub latency: LatencyMetrics,
    /// 连接指标
    pub connection: ConnectionMetrics,
    /// 基准测试结果
    pub benchmarks: HashMap<String, BenchmarkResult>,
    /// 报告生成时间
    pub generated_at: chrono::DateTime<chrono::Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_performance_monitor_creation() {
        let monitor = PerformanceMonitor::new();
        
        // 测试初始状态
        let network = monitor.network_metrics.read().await;
        assert_eq!(network.bytes_sent, 0);
        assert_eq!(network.packets_sent, 0);
    }

    #[tokio::test]
    async fn test_network_metrics_recording() {
        let monitor = PerformanceMonitor::new();
        
        // 记录网络活动
        monitor.record_network_send(1024).await;
        monitor.record_network_receive(512).await;
        monitor.record_network_error(true).await;
        
        let metrics = monitor.network_metrics.read().await;
        assert_eq!(metrics.bytes_sent, 1024);
        assert_eq!(metrics.bytes_received, 512);
        assert_eq!(metrics.packets_sent, 1);
        assert_eq!(metrics.packets_received, 1);
        assert_eq!(metrics.send_errors, 1);
    }

    #[tokio::test]
    async fn test_latency_metrics_recording() {
        let monitor = PerformanceMonitor::new();
        
        // 记录一些延迟数据
        monitor.record_latency(10.0).await;
        monitor.record_latency(20.0).await;
        monitor.record_latency(30.0).await;
        
        let metrics = monitor.latency_metrics.read().await;
        assert_eq!(metrics.sample_count, 3);
        assert_eq!(metrics.min_latency, 10.0);
        assert_eq!(metrics.max_latency, 30.0);
        assert_eq!(metrics.average_latency, 20.0);
    }

    #[tokio::test]
    async fn test_throughput_benchmark() {
        let monitor = PerformanceMonitor::new();
        let test_suite = PerformanceTestSuite {
            concurrency: 1,
            duration_seconds: 1,
            packet_size: 100,
            test_interval_ms: 10,
        };
        
        let result = monitor.run_throughput_benchmark("test_throughput", &test_suite).await;
        assert!(result.is_ok());
        
        let result = result.unwrap();
        assert_eq!(result.name, "test_throughput");
        assert!(result.operations > 0);
        assert!(result.ops_per_second > 0.0);
    }

    #[tokio::test]
    async fn test_latency_benchmark() {
        let monitor = PerformanceMonitor::new();
        
        let result = monitor.run_latency_benchmark("test_latency", 10).await;
        assert!(result.is_ok());
        
        let result = result.unwrap();
        assert_eq!(result.name, "test_latency");
        assert_eq!(result.operations, 10);
        assert!(result.average_latency > 0.0);
    }

    #[tokio::test]
    async fn test_connection_metrics() {
        let monitor = PerformanceMonitor::new();
        
        // 模拟连接事件
        monitor.record_connection_event(ConnectionEvent::Connected, None).await;
        monitor.record_connection_event(ConnectionEvent::Connected, None).await;
        monitor.record_connection_event(ConnectionEvent::Disconnected, Some(10.0)).await;
        monitor.record_connection_event(ConnectionEvent::Failed, None).await;
        
        let metrics = monitor.connection_metrics.read().await;
        assert_eq!(metrics.active_connections, 1);
        assert_eq!(metrics.total_connections, 2);
        assert_eq!(metrics.failed_connections, 1);
        assert!(metrics.connection_success_rate > 0.0);
    }

    #[tokio::test]
    async fn test_performance_report_generation() {
        let monitor = PerformanceMonitor::new();
        
        // 添加一些数据
        monitor.record_network_send(1024).await;
        monitor.record_latency(15.5).await;
        
        let report = monitor.generate_report().await;
        assert!(report.contains("性能监控报告"));
        assert!(report.contains("网络性能"));
        assert!(report.contains("内存性能"));
        assert!(report.contains("延迟性能"));
    }
}