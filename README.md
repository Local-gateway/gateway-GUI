# WDIC 网关 (WDIC Gateway)

[![CI](https://github.com/Local-gateway/gateway/actions/workflows/ci.yml/badge.svg)](https://github.com/Local-gateway/gateway/actions/workflows/ci.yml)
[![Mobile Builds](https://github.com/Local-gateway/gateway/actions/workflows/mobile.yml/badge.svg)](https://github.com/Local-gateway/gateway/actions/workflows/mobile.yml)
[![Performance Tests](https://github.com/Local-gateway/gateway/actions/workflows/performance.yml/badge.svg)](https://github.com/Local-gateway/gateway/actions/workflows/performance.yml)
[![Release](https://github.com/Local-gateway/gateway/actions/workflows/release.yml/badge.svg)](https://github.com/Local-gateway/gateway/actions/workflows/release.yml)

一个基于 QUIC 协议的跨平台本地网关实现，提供 P2P 网络发现和注册表管理功能。**新增基于 UDP 的 WDIC 协议自主广播功能**，所有网关都是一等公民。支持多平台编译和自动化性能测试。

## 平台支持

WDIC Gateway 支持所有主流平台和架构的交叉编译：

### 桌面平台
- **Linux**: x86_64, aarch64, armv7
- **Windows**: x86_64
- **macOS**: x86_64, Apple Silicon (aarch64)

### 移动平台
- **Android**: armv7, aarch64, i686, x86_64
- **iOS**: aarch64 (设备), x86_64 (模拟器)
- **HarmonyOS**: aarch64, armv7 (兼容 Android 目标)

### 自动化构建

所有平台的二进制文件通过 GitHub Actions 自动构建：
- **持续集成**: 每次推送和 PR 自动构建和测试
- **性能测试**: 自动化性能基准测试和压力测试
- **发布构建**: 标签推送时自动构建所有平台版本
- **每日测试**: 定时运行性能回归测试

### 二进制下载

预编译的二进制文件可在 [Releases](https://github.com/Local-gateway/gateway/releases) 页面下载。

## 功能特性

- 🔐 **安全通信**: 基于 QUIC 协议的 WDIC (Web Dynamic Inter-Connection) 网络协议
- 🚀 **UDP 广播**: 新增基于 UDP 的自主广播协议，支持定向和全网广播
- 📁 **目录挂载**: 目录索引生成和二进制文件管理，支持文件搜索
- 📤 **文件传输**: 安全的文件发送和接收功能
- 💬 **信息传递**: 网关间的实时信息广播和通信
- 🔍 **智能搜索**: 分布式文件搜索和目录查询功能
- 🏷️ **令牌体系**: 独特的令牌通信机制，支持多种操作类型
- ⚡ **性能测试**: 内置性能测试和瓶颈分析工具
- 📝 **注册表管理**: 自动维护网络中其他网关的注册信息
- 📡 **P2P 广播**: 局域网内的自动发现和广播功能
- 🔄 **实时同步**: 网关间的实时状态同步和心跳检测
- 🏠 **本地服务**: 在 55555 端口提供网关服务，UDP广播固定使用 55556 端口
- 🧪 **完整测试**: 100% 测试驱动开发，确保代码质量

## 架构设计

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   网关 A        │    │   网关 B        │    │   网关 C        │
│ QUIC: 55555     │    │ QUIC: 55555     │    │ QUIC: 55555     │
│ UDP:  55556     │    │ UDP:  55556     │    │ UDP:  55556     │
└─────────────────┘    └─────────────────┘    └─────────────────┘
         │                       │                       │
         └───────────────────────┼───────────────────────┘
                                 │
                    ┌─────────────────┐
                    │  双协议支持     │
                    │  QUIC + UDP     │
                    └─────────────────┘
                                 │
                    ┌─────────────────┐
                    │   令牌体系      │
                    │ (目录/文件/消息) │
                    └─────────────────┘
```

## 新增 UDP 协议功能

### 令牌类型

1. **DirectorySearch** - 目录搜索令牌
2. **DirectorySearchResponse** - 目录搜索响应
3. **FileRequest** - 文件请求令牌
4. **FileResponse** - 文件响应令牌
5. **InfoMessage** - 信息消息令牌
6. **PerformanceTest** - 性能测试令牌

### 核心特性

- **隐蔽异常处理**: 自动隐蔽和处理未监听端口的 OS 异常
- **定向广播**: 支持指定 IP 和端口的精确投递
- **目录挂载**: 自动生成目录索引，支持二进制格式保存
- **分布式搜索**: 跨网关的文件搜索和发现
- **性能监控**: 实时性能测试和瓶颈分析

## 快速开始

### 前置要求

- Rust 1.89.0 或更高版本
- 网络权限（用于 UDP 广播）

### 从源码构建

1. 克隆仓库：
```bash
git clone https://github.com/Local-gateway/gateway.git
cd gateway
```

2. 构建项目：
```bash
cargo build --release
```

3. 运行网关：
```bash
cargo run
```

或者设置日志级别：
```bash
RUST_LOG=info cargo run
```

### 交叉编译

项目支持多种目标平台的交叉编译：

#### Android 平台
```bash
# 安装 Android NDK 和目标
rustup target add aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android

# 设置环境变量（以 aarch64 为例）
export ANDROID_NDK_ROOT=/path/to/android-ndk
export CC_aarch64_linux_android=$ANDROID_NDK_ROOT/toolchains/llvm/prebuilt/linux-x86_64/bin/aarch64-linux-android21-clang
export AR_aarch64_linux_android=$ANDROID_NDK_ROOT/toolchains/llvm/prebuilt/linux-x86_64/bin/llvm-ar

# 构建
cargo build --target aarch64-linux-android --release
```

#### iOS 平台
```bash
# 安装目标
rustup target add aarch64-apple-ios x86_64-apple-ios

# 构建
cargo build --target aarch64-apple-ios --release
```

#### Linux ARM 平台
```bash
# 安装交叉编译工具
sudo apt-get install gcc-aarch64-linux-gnu gcc-arm-linux-gnueabihf

# 安装目标
rustup target add aarch64-unknown-linux-gnu armv7-unknown-linux-gnueabihf

# 构建
cargo build --target aarch64-unknown-linux-gnu --release
```

#### Windows 平台
```bash
# 安装目标
rustup target add x86_64-pc-windows-msvc

# 构建
cargo build --target x86_64-pc-windows-msvc --release
```

#### macOS 平台
```bash
# 安装目标
rustup target add x86_64-apple-darwin aarch64-apple-darwin

# 构建
cargo build --target aarch64-apple-darwin --release
```

## UDP 固定端口功能

从 v0.2.1 版本开始，WDIC Gateway 使用固定端口 **55556** 进行 UDP 广播通信：

### 主要特性
- **固定端口**: 所有 UDP 通信统一使用端口 55556
- **智能实例管理**: 自动检测端口占用，支持连接到现有实例
- **错误处理**: 清晰的错误提示和处理机制

### 使用方法
```rust
use wdic_gateway::udp_protocol::UdpBroadcastManager;

// 推荐：创建或连接到现有实例
let manager = UdpBroadcastManager::new_or_connect().await?;

// 指定IP地址
use std::net::{IpAddr, Ipv4Addr};
let ip = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
let manager = UdpBroadcastManager::new_or_connect_with_ip(ip).await?;
```

### 网络配置
确保防火墙允许 UDP 端口 55556：
```bash
# Windows
netsh advfirewall firewall add rule name="WDIC UDP" dir=in action=allow protocol=UDP localport=55556

# Linux
sudo ufw allow 55556/udp
```

详细说明请参考 [UDP_FIXED_PORT.md](UDP_FIXED_PORT.md)。

### 配置

网关支持通过配置文件或环境变量进行自定义配置：

```rust
use wdic_gateway::{Gateway, GatewayConfig};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = GatewayConfig {
        name: "我的网关".to_string(),
        port: 55555,
        broadcast_interval: 30,  // 广播间隔（秒）
        heartbeat_interval: 60,  // 心跳间隔（秒）
        connection_timeout: 300, // 连接超时（秒）
        registry_cleanup_interval: 120, // 注册表清理间隔（秒）
    };
    
    let gateway = Gateway::with_config(config).await?;
    gateway.run().await?;
    Ok(())
}
```

## API 文档

### 核心模块

#### `Gateway` - 网关主类
负责协调各个模块的工作，是网关的核心控制器。

```rust
// 创建新网关
let gateway = Gateway::new("网关名称".to_string()).await?;

// 启动网关服务
gateway.run().await?;

// 获取网关统计信息
let (registry_size, active_connections) = gateway.get_stats().await;

// 挂载目录
gateway.mount_directory("docs".to_string(), "/path/to/docs".to_string()).await?;

// 广播搜索请求
let sent_count = gateway.broadcast_directory_search(vec!["pdf".to_string()]).await?;

// 发送信息消息
let sent_count = gateway.broadcast_info_message("Hello Network!".to_string()).await?;

// 性能测试
let latency = gateway.run_performance_test("latency".to_string(), 1024).await?;
```

#### `UdpBroadcastManager` - UDP 广播管理器
处理基于 UDP 的令牌广播和文件操作。

```rust
// 创建 UDP 广播管理器
let manager = UdpBroadcastManager::new(local_addr)?;

// 广播令牌
let token = UdpToken::InfoMessage { /* ... */ };
manager.broadcast_token(&token).await?;

// 定向发送令牌
manager.send_token_to(&token, target_addr).await?;

// 挂载目录
manager.mount_directory("share".to_string(), "/path/to/share".to_string()).await?;

// 搜索文件
let results = manager.search_files(&["keyword".to_string()]).await;
```

#### `DirectoryIndex` - 目录索引
管理目录结构和文件索引。

```rust
// 生成目录索引
let index = DirectoryIndex::generate("/path/to/directory")?;

// 保存索引到文件
index.save_to_file("directory.index")?;

// 从文件加载索引
let index = DirectoryIndex::load_from_file("directory.index")?;

// 搜索文件
let matches = index.search(&["keyword".to_string()]);
```

#### `Registry` - 注册表管理
管理网络中所有已知网关的注册信息。

```rust
// 创建注册表
let registry = Registry::new("本地网关".to_string(), local_addr);

// 添加网关条目
registry.add_or_update(entry);

// 获取所有条目
let entries = registry.all_entries();
```

#### `WdicMessage` - 协议消息
定义 WDIC 协议的各种消息类型。

```rust
// 创建广播消息
let message = WdicMessage::broadcast(local_entry);

// 序列化消息
let bytes = message.to_bytes()?;

// 反序列化消息
let message = WdicMessage::from_bytes(&bytes)?;
```

#### `NetworkManager` - 网络管理
处理网络通信、UDP 广播和连接管理。

```rust
// 创建网络管理器
let manager = NetworkManager::new(local_addr)?;

// 广播消息
manager.broadcast_message(&message).await?;

// 发送消息到指定地址
manager.send_message(&message, target_addr).await?;
```

## 协议规范

### WDIC 消息类型

1. **Broadcast** - 广播消息
   - 用于向网络宣告自己的存在
   - 包含发送者的完整信息

2. **BroadcastResponse** - 广播响应
   - 响应广播消息
   - 返回已知的其他网关列表

3. **Heartbeat** - 心跳消息
   - 保持连接活跃状态
   - 定期发送以检测网关可用性

4. **RegisterRequest** - 注册请求
   - 请求加入网络
   - 显式注册网关信息

5. **QueryGateways** - 查询网关
   - 查询当前网络中的所有网关
   - 用于网络拓扑发现

### 消息格式

所有消息使用 JSON 格式进行序列化：

```json
{
  "Broadcast": {
    "sender": {
      "id": "550e8400-e29b-41d4-a716-446655440000",
      "name": "本地网关",
      "address": "192.168.1.100:55555",
      "last_seen": "2024-01-01T12:00:00Z"
    }
  }
}
```

## UDP 协议规范

### 令牌消息格式

所有 UDP 令牌使用 JSON 格式进行序列化：

```json
{
  "InfoMessage": {
    "sender_id": "550e8400-e29b-41d4-a716-446655440000",
    "content": "Hello Network!",
    "message_id": "550e8400-e29b-41d4-a716-446655440001"
  }
}
```

### 文件搜索示例

```json
{
  "DirectorySearch": {
    "searcher_id": "550e8400-e29b-41d4-a716-446655440000",
    "keywords": ["pdf", "document"],
    "search_id": "550e8400-e29b-41d4-a716-446655440001"
  }
}
```

### 性能测试令牌

```json
{
  "PerformanceTest": {
    "tester_id": "550e8400-e29b-41d4-a716-446655440000",
    "test_type": "latency_test",
    "data_size": 1024,
    "start_time": "2024-01-01T12:00:00Z"
  }
}
```

## 开发指南

### CI/CD 工作流

项目使用 GitHub Actions 实现完整的 CI/CD 流程：

#### 1. 持续集成 (CI)
- **触发条件**: 推送到 main/develop 分支或创建 PR
- **任务**: 
  - 代码质量检查 (clippy, fmt)
  - 单元测试和集成测试
  - 多平台构建验证

#### 2. 移动平台构建 (Mobile)
- **触发条件**: 推送到 main/develop 分支或手动触发
- **平台**: Android (4个架构), iOS (2个架构), HarmonyOS (2个架构)
- **产物**: 所有平台的二进制文件

#### 3. 性能测试 (Performance)
- **触发条件**: 推送、PR、每日定时 (UTC 2:00AM) 或手动触发
- **测试类型**:
  - 基准测试 (Criterion)
  - 压力测试 (多线程、高负载)
  - 内存使用分析 (Valgrind)
  - 网络性能测试
  - PR 性能对比

#### 4. 发布构建 (Release)
- **触发条件**: 推送版本标签或手动触发
- **产物**: 所有支持平台的发布版本二进制文件
- **目标**: 13个不同的平台/架构组合

### 运行演示

```bash
# 运行 UDP 协议演示
cargo run --example udp_demo

# 运行基础网关演示
cargo run --example basic_usage

# 运行缓存演示
cargo run --example cache_demo

# 运行性能演示
cargo run --example performance_demo
```

### 运行测试

```bash
# 运行所有测试
cargo test

# 运行 UDP 协议测试
cargo test udp_protocol

# 运行网关测试
cargo test gateway

# 运行压力测试
cargo test --test stress_tests

# 运行集成测试
cargo test --test integration_tests

# 运行测试并显示输出
cargo test -- --nocapture
```

### 性能基准测试

```bash
# 运行所有基准测试
cargo bench

# 运行特定基准测试
cargo bench --bench performance_benchmarks

# 生成基准测试报告
cargo bench --bench performance_benchmarks -- --output-format html
```

### 生成文档

```bash
# 生成 API 文档
cargo doc --no-deps --open

# 生成所有依赖的文档
cargo doc --open
```

### 代码质量检查

```bash
# 格式化代码
cargo fmt

# 检查代码质量
cargo clippy

# 严格模式检查
cargo clippy -- -D warnings

# 检查未使用的依赖
cargo machete
```

### 内存和性能分析

```bash
# 使用 Valgrind 进行内存检查
valgrind --tool=memcheck --leak-check=full target/debug/wdic-gateway

# 使用 perf 进行性能分析
perf record target/release/wdic-gateway
perf report

# 使用 flamegraph 生成火焰图
cargo install flamegraph
cargo flamegraph --bin wdic-gateway
```

### 添加新功能

1. **编写测试**: 遵循 TDD 原则，先编写测试
2. **实现功能**: 编写最小可行的实现
3. **文档更新**: 更新 API 文档和注释
4. **集成测试**: 确保新功能与现有系统兼容

## 性能特性

- **低延迟**: 基于 QUIC 协议的高效网络通信
- **高并发**: 异步 I/O 支持大量并发连接
- **内存效率**: 精心设计的数据结构，最小化内存占用
- **网络优化**: 智能广播策略，减少网络流量
- **跨平台优化**: 针对不同平台和架构的性能优化

### 性能监控

项目内置完整的性能监控系统：

- **延迟监控**: 实时监控网络延迟和响应时间
- **吞吐量监控**: 监控数据传输速率和处理能力
- **内存使用监控**: 跟踪内存分配和释放
- **连接状态监控**: 监控活跃连接数和连接质量
- **错误率监控**: 统计各种类型的错误和异常

### 基准测试结果

项目包含全面的基准测试套件，涵盖：

- **网络性能**: UDP 广播、QUIC 连接、P2P 发现
- **数据处理**: 压缩、解压、序列化、反序列化
- **缓存性能**: 缓存命中率、查询速度、清理效率
- **并发性能**: 多线程处理、异步 I/O、锁竞争
- **文件操作**: 目录索引、文件搜索、传输速度

所有基准测试结果通过 GitHub Actions 自动生成和发布。

## 安全考虑

- **协议验证**: 严格的消息格式验证
- **连接管理**: 自动清理过期连接，防止资源泄露
- **错误处理**: 完整的错误处理机制
- **日志记录**: 详细的操作日志，便于监控和调试

## 故障排除

### 常见问题

1. **端口占用**
   ```bash
   # 检查端口占用
   netstat -tulpn | grep 55555
   # 或者使用 ss
   ss -tulpn | grep 55555
   ```

2. **广播权限**
   ```bash
   # 确保程序有网络广播权限
   # 在某些环境中可能需要管理员权限
   sudo setcap cap_net_raw,cap_net_admin=eip target/release/wdic-gateway
   ```

3. **防火墙配置**
   ```bash
   # Ubuntu/Debian
   sudo ufw allow 55555/udp
   sudo ufw allow 55556/udp
   
   # CentOS/RHEL
   sudo firewall-cmd --permanent --add-port=55555/udp
   sudo firewall-cmd --permanent --add-port=55556/udp
   sudo firewall-cmd --reload
   
   # Windows
   netsh advfirewall firewall add rule name="WDIC Gateway UDP" dir=in action=allow protocol=UDP localport=55555-55556
   ```

4. **交叉编译问题**
   ```bash
   # Android 交叉编译失败
   export ANDROID_NDK_ROOT=/path/to/ndk
   export PATH=$ANDROID_NDK_ROOT/toolchains/llvm/prebuilt/linux-x86_64/bin:$PATH
   
   # iOS 交叉编译失败 (需要 macOS)
   xcode-select --install
   
   # Linux ARM 交叉编译失败
   sudo apt-get install gcc-multilib gcc-aarch64-linux-gnu
   ```

5. **性能问题诊断**
   ```bash
   # 启用详细日志
   RUST_LOG=debug cargo run
   
   # 运行性能分析
   cargo bench --bench performance_benchmarks
   
   # 内存泄漏检查
   valgrind --tool=memcheck --leak-check=full target/debug/wdic-gateway
   ```

6. **网络连接问题**
   ```bash
   # 检查网络接口
   ip addr show
   
   # 测试广播连通性
   cargo test --test integration_tests test_udp_broadcast_functionality -- --nocapture
   
   # 检查 QUIC 连接
   cargo test --test integration_tests test_p2p_discovery -- --nocapture
   ```

### 构建问题

1. **依赖下载失败**
   ```bash
   # 使用国内镜像源
   echo '[source.crates-io]
   replace-with = "rsproxy"
   [source.rsproxy]
   registry = "https://rsproxy.cn/crates.io-index"' >> ~/.cargo/config.toml
   ```

2. **链接错误**
   ```bash
   # 安装必要的系统依赖
   # Ubuntu/Debian
   sudo apt-get install build-essential pkg-config libssl-dev
   
   # CentOS/RHEL
   sudo yum groupinstall "Development Tools"
   sudo yum install pkgconfig openssl-devel
   
   # macOS
   xcode-select --install
   brew install pkg-config openssl
   ```

3. **目标平台不支持**
   ```bash
   # 查看所有可用目标
   rustup target list
   
   # 安装特定目标
   rustup target add aarch64-unknown-linux-gnu
   ```

### 调试技巧

1. **启用调试日志**
   ```bash
   export RUST_LOG=wdic_gateway=debug
   cargo run
   ```

2. **使用调试器**
   ```bash
   # GDB
   cargo build
   gdb target/debug/wdic-gateway
   
   # LLDB (macOS)
   cargo build
   lldb target/debug/wdic-gateway
   ```

3. **网络抓包分析**
   ```bash
   # 使用 tcpdump
   sudo tcpdump -i any -w capture.pcap port 55555 or port 55556
   
   # 使用 Wireshark
   wireshark -i any -f "port 55555 or port 55556"
   ```

## 贡献指南

1. Fork 这个仓库
2. 创建功能分支 (`git checkout -b feature/amazing-feature`)
3. 提交更改 (`git commit -m 'Add some amazing feature'`)
4. 推送到分支 (`git push origin feature/amazing-feature`)
5. 开启 Pull Request

## 许可证

本项目采用 MIT 许可证 - 查看 [LICENSE](LICENSE) 文件了解详情。

## 更新日志

### v0.3.0 (计划中)

#### 新增功能
- 🏗️ **跨平台构建系统**: 支持 13 种平台/架构组合的自动化构建
- 🤖 **CI/CD 工作流**: 完整的 GitHub Actions 工作流程
- 📊 **自动化性能测试**: 持续性能监控和回归测试
- 📱 **移动平台支持**: Android、iOS、HarmonyOS 完整支持
- 🔄 **每日构建**: 定时构建和测试确保代码质量
- 📈 **性能对比**: PR 自动性能对比和分析
- 🛠️ **开发工具链**: 完整的开发和调试工具集成

#### 技术改进
- 跨平台编译优化和构建脚本
- 内存使用分析和优化工具集成
- 网络性能基准测试套件
- 多架构二进制文件自动发布
- 详细的故障排除和调试指南
- 移除 Windows 32位 (i686) 支持，简化构建系统

#### API 变更
- 保持 v0.2.0 API 完全兼容
- 新增平台特定的配置选项
- 增强性能监控和度量 API
- 优化构建配置和依赖管理

### v0.2.0

#### 新增功能
- ✨ **UDP 广播协议**: 实现基于 UDP 的 WDIC 协议自主广播
- 📁 **目录挂载系统**: 支持目录索引生成和二进制文件管理  
- 🔍 **分布式文件搜索**: 跨网关的智能文件发现功能
- 📤 **文件传输**: 安全的文件发送和接收机制
- 💬 **信息广播**: 实时信息传递和通信功能
- 🏷️ **令牌体系**: 独特的令牌通信协议，支持多种操作类型
- ⚡ **性能测试**: 内置性能监控和瓶颈分析工具
- 🎯 **定向广播**: 支持指定 IP 和端口的精确消息投递
- 🛡️ **异常隐蔽**: 自动处理和隐蔽未监听端口的 OS 异常
- 🧪 **完整测试**: 新增 10+ 个测试用例，覆盖所有新功能

#### 技术改进
- 双协议支持：QUIC（安全通信）+ UDP（快速广播）
- 所有网关均为一等公民，无主从关系
- 二进制文件格式的目录索引系统
- Base64 编码的安全文件传输
- JSON 格式的令牌序列化
- 自动端口分配和冲突避免

#### API 变更
- 新增 `UdpBroadcastManager` 类
- 新增 `DirectoryIndex` 目录索引管理
- 新增 `UdpToken` 枚举类型，支持 6 种令牌
- `Gateway` 类新增目录、搜索、文件、性能测试相关方法
- 保持原有 QUIC 协议 API 完全兼容

### v0.1.0

- ✨ 初始版本发布
- 🚀 基于 QUIC 的 WDIC 协议实现
- 📝 完整的注册表管理功能
- 📡 P2P 网络发现和广播
- 🧪 100% 测试覆盖
- 📚 完整的 API 文档

## 联系方式

- 项目主页: https://github.com/Local-gateway/gateway
- 问题反馈: https://github.com/Local-gateway/gateway/issues
- 文档: https://local-gateway.github.io/gateway/
