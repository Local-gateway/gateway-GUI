# API 文档 (API Documentation)

WDIC Gateway 提供了完整的 Rust API，支持 P2P 网络发现、文件传输、性能监控等功能。

## 核心模块

### Gateway - 网关主类

网关的核心控制器，负责协调各个模块的工作。

```rust
use wdic_gateway::{Gateway, GatewayConfig};

// 创建默认配置的网关
let gateway = Gateway::new("我的网关".to_string()).await?;

// 创建自定义配置的网关
let config = GatewayConfig {
    name: "自定义网关".to_string(),
    port: 55555,
    broadcast_interval: 30,
    heartbeat_interval: 60,
    connection_timeout: 300,
    registry_cleanup_interval: 120,
};
let gateway = Gateway::with_config(config).await?;
```

#### 核心方法

##### `new(name: String) -> Result<Gateway>`
创建新的网关实例。

**参数:**
- `name`: 网关名称

**返回值:**
- `Result<Gateway>`: 成功时返回网关实例

##### `with_config(config: GatewayConfig) -> Result<Gateway>`
使用自定义配置创建网关实例。

**参数:**
- `config`: 网关配置

##### `run() -> Result<()>`
启动网关服务，开始监听和处理网络请求。

##### `stop() -> Result<()>`
停止网关服务，清理资源。

##### `get_stats() -> (usize, usize)`
获取网关统计信息。

**返回值:**
- `(注册表大小, 活跃连接数)`

#### 目录和文件操作

##### `mount_directory(name: String, path: String) -> Result<()>`
挂载目录到网关，生成索引并支持搜索。

**参数:**
- `name`: 目录别名
- `path`: 实际目录路径

```rust
gateway.mount_directory("共享文档".to_string(), "/home/user/documents".to_string()).await?;
```

##### `broadcast_directory_search(keywords: Vec<String>) -> Result<usize>`
广播目录搜索请求到网络中的所有网关。

**参数:**
- `keywords`: 搜索关键词列表

**返回值:**
- `Result<usize>`: 发送的消息数量

```rust
let sent_count = gateway.broadcast_directory_search(vec!["pdf".to_string(), "文档".to_string()]).await?;
```

#### 信息传递

##### `broadcast_info_message(content: String) -> Result<usize>`
向网络广播信息消息。

**参数:**
- `content`: 消息内容

```rust
let sent_count = gateway.broadcast_info_message("Hello Network!".to_string()).await?;
```

#### 性能测试

##### `run_performance_test(test_type: String, data_size: usize) -> Result<Duration>`
运行性能测试并返回延迟。

**参数:**
- `test_type`: 测试类型 ("latency_test", "throughput_test", etc.)
- `data_size`: 测试数据大小

```rust
let latency = gateway.run_performance_test("latency_test".to_string(), 1024).await?;
println!("延迟: {:?}", latency);
```

### UdpBroadcastManager - UDP 广播管理器

处理基于 UDP 的令牌广播和文件操作。

```rust
use wdic_gateway::UdpBroadcastManager;
use std::net::SocketAddr;

let local_addr: SocketAddr = "127.0.0.1:55556".parse()?;
let manager = UdpBroadcastManager::new(local_addr)?;
```

#### 核心方法

##### `new(local_addr: SocketAddr) -> Result<UdpBroadcastManager>`
创建 UDP 广播管理器。

##### `broadcast_token(token: &UdpToken) -> Result<usize>`
广播令牌到网络中的所有网关。

```rust
use wdic_gateway::UdpToken;

let token = UdpToken::InfoMessage {
    sender_id: uuid::Uuid::new_v4(),
    content: "Hello!".to_string(),
    message_id: uuid::Uuid::new_v4(),
};

let sent_count = manager.broadcast_token(&token).await?;
```

##### `send_token_to(token: &UdpToken, target: SocketAddr) -> Result<()>`
向指定地址发送令牌。

##### `mount_directory(name: String, path: String) -> Result<()>`
挂载目录并生成索引。

##### `search_files(keywords: &[String]) -> Vec<String>`
在已挂载的目录中搜索文件。

### DirectoryIndex - 目录索引

管理目录结构和文件索引。

```rust
use wdic_gateway::DirectoryIndex;

// 生成目录索引
let index = DirectoryIndex::generate("/path/to/directory")?;

// 保存到文件
index.save_to_file("directory.index")?;

// 从文件加载
let loaded_index = DirectoryIndex::load_from_file("directory.index")?;

// 搜索文件
let matches = index.search(&["keyword".to_string()]);
```

#### 核心方法

##### `generate(path: &str) -> Result<DirectoryIndex>`
生成指定路径的目录索引。

##### `save_to_file(&self, file_path: &str) -> Result<()>`
将索引保存到二进制文件。

##### `load_from_file(file_path: &str) -> Result<DirectoryIndex>`
从二进制文件加载索引。

##### `search(&self, keywords: &[String]) -> Vec<String>`
搜索匹配关键词的文件。

### Registry - 注册表管理

管理网络中所有已知网关的注册信息。

```rust
use wdic_gateway::{Registry, RegistryEntry};
use std::net::SocketAddr;

let local_addr: SocketAddr = "127.0.0.1:55555".parse()?;
let registry = Registry::new("本地网关".to_string(), local_addr);
```

#### 核心方法

##### `new(name: String, address: SocketAddr) -> Registry`
创建新的注册表。

##### `add_or_update(&self, entry: RegistryEntry)`
添加或更新网关条目。

##### `all_entries(&self) -> Vec<RegistryEntry>`
获取所有注册的网关条目。

##### `get_by_address(&self, address: &SocketAddr) -> Option<RegistryEntry>`
根据地址获取网关条目。

##### `remove(&self, address: &SocketAddr) -> bool`
移除指定地址的网关条目。

##### `cleanup_expired(&self, timeout_seconds: u64) -> usize`
清理过期的网关条目。

### NetworkManager - 网络管理

处理网络通信、UDP 广播和连接管理。

```rust
use wdic_gateway::NetworkManager;
use std::net::SocketAddr;

let local_addr: SocketAddr = "127.0.0.1:55555".parse()?;
let manager = NetworkManager::new(local_addr)?;
```

#### 核心方法

##### `new(local_addr: SocketAddr) -> Result<NetworkManager>`
创建网络管理器。

##### `broadcast_message(message: &WdicMessage) -> Result<usize>`
广播消息到网络。

##### `send_message(message: &WdicMessage, target: SocketAddr) -> Result<()>`
发送消息到指定地址。

### PerformanceMonitor - 性能监控

实时性能监控和度量。

```rust
use wdic_gateway::PerformanceMonitor;

let monitor = PerformanceMonitor::new();

// 记录延迟
monitor.record_latency("udp_broadcast", Duration::from_millis(10));

// 记录吞吐量
monitor.record_throughput("file_transfer", 1_000_000); // bytes per second

// 生成报告
let report = monitor.generate_report();
println!("{}", report);
```

#### 核心方法

##### `new() -> PerformanceMonitor`
创建性能监控器。

##### `record_latency(&self, operation: &str, latency: Duration)`
记录操作延迟。

##### `record_throughput(&self, operation: &str, bytes_per_second: u64)`
记录吞吐量。

##### `generate_report(&self) -> String`
生成性能报告。

## 数据类型

### UdpToken - UDP 令牌

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UdpToken {
    DirectorySearch {
        searcher_id: Uuid,
        keywords: Vec<String>,
        search_id: Uuid,
    },
    DirectorySearchResponse {
        responder_id: Uuid,
        search_id: Uuid,
        results: Vec<String>,
    },
    FileRequest {
        requester_id: Uuid,
        file_path: String,
        request_id: Uuid,
    },
    FileResponse {
        responder_id: Uuid,
        request_id: Uuid,
        file_data: String, // Base64 编码
        file_name: String,
    },
    InfoMessage {
        sender_id: Uuid,
        content: String,
        message_id: Uuid,
    },
    PerformanceTest {
        tester_id: Uuid,
        test_type: String,
        data_size: usize,
        start_time: DateTime<Utc>,
    },
}
```

### WdicMessage - WDIC 协议消息

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WdicMessage {
    Broadcast { sender: RegistryEntry },
    BroadcastResponse { 
        sender: RegistryEntry, 
        known_gateways: Vec<RegistryEntry> 
    },
    Heartbeat { sender: RegistryEntry },
    RegisterRequest { requester: RegistryEntry },
    QueryGateways { requester: RegistryEntry },
}
```

### RegistryEntry - 注册表条目

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryEntry {
    pub id: Uuid,
    pub name: String,
    pub address: SocketAddr,
    pub last_seen: DateTime<Utc>,
}
```

### GatewayConfig - 网关配置

```rust
#[derive(Debug, Clone)]
pub struct GatewayConfig {
    pub name: String,
    pub port: u16,
    pub broadcast_interval: u64,     // 秒
    pub heartbeat_interval: u64,     // 秒
    pub connection_timeout: u64,     // 秒
    pub registry_cleanup_interval: u64, // 秒
}
```

## 错误处理

所有 API 方法使用 `anyhow::Result` 进行错误处理：

```rust
use anyhow::Result;

fn example_function() -> Result<()> {
    let gateway = Gateway::new("test".to_string()).await?;
    gateway.run().await?;
    Ok(())
}
```

常见错误类型：
- 网络连接错误
- 文件系统错误
- 序列化/反序列化错误
- 配置错误

## 示例用法

### 基础网关使用

```rust
use wdic_gateway::{Gateway, GatewayConfig};
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    // 创建网关
    let gateway = Gateway::new("我的网关".to_string()).await?;
    
    // 挂载目录
    gateway.mount_directory(
        "文档".to_string(), 
        "/home/user/documents".to_string()
    ).await?;
    
    // 启动网关
    gateway.run().await?;
    
    Ok(())
}
```

### UDP 广播使用

```rust
use wdic_gateway::{UdpBroadcastManager, UdpToken};
use std::net::SocketAddr;
use uuid::Uuid;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let local_addr: SocketAddr = "0.0.0.0:55556".parse()?;
    let manager = UdpBroadcastManager::new(local_addr)?;
    
    // 广播信息消息
    let token = UdpToken::InfoMessage {
        sender_id: Uuid::new_v4(),
        content: "Hello Network!".to_string(),
        message_id: Uuid::new_v4(),
    };
    
    let sent_count = manager.broadcast_token(&token).await?;
    println!("发送了 {} 条消息", sent_count);
    
    Ok(())
}
```

### 性能监控使用

```rust
use wdic_gateway::PerformanceMonitor;
use std::time::{Duration, Instant};

fn main() {
    let monitor = PerformanceMonitor::new();
    
    // 测量操作性能
    let start = Instant::now();
    // ... 执行某些操作 ...
    let duration = start.elapsed();
    
    monitor.record_latency("my_operation", duration);
    
    // 生成报告
    let report = monitor.generate_report();
    println!("{}", report);
}
```

这套 API 提供了完整的网关功能，支持跨平台部署和高性能网络通信。