# WDIC 网关 (WDIC Gateway)

一个基于 QUIC 协议的本地网关实现，提供 P2P 网络发现和注册表管理功能。

## 功能特性

- 🔐 **安全通信**: 基于 QUIC 协议的 WDIC (Web Dynamic Inter-Connection) 网络协议
- 📝 **注册表管理**: 自动维护网络中其他网关的注册信息
- 📡 **P2P 广播**: 局域网内的自动发现和广播功能
- 🔄 **实时同步**: 网关间的实时状态同步和心跳检测
- 🏠 **本地服务**: 在 55555 端口提供网关服务
- 🧪 **完整测试**: 100% 测试驱动开发，确保代码质量

## 架构设计

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   网关 A        │    │   网关 B        │    │   网关 C        │
│   (55555)       │    │   (55555)       │    │   (55555)       │
└─────────────────┘    └─────────────────┘    └─────────────────┘
         │                       │                       │
         └───────────────────────┼───────────────────────┘
                                 │
                    ┌─────────────────┐
                    │  WDIC 协议层    │
                    │  (基于 QUIC)    │
                    └─────────────────┘
                                 │
                    ┌─────────────────┐
                    │   UDP 广播      │
                    │  (局域网发现)   │
                    └─────────────────┘
```

## 快速开始

### 前置要求

- Rust 1.89.0 或更高版本
- 网络权限（用于 UDP 广播）

### 安装和运行

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

## 开发指南

### 运行测试

```bash
# 运行所有测试
cargo test

# 运行特定模块测试
cargo test registry

# 运行测试并显示输出
cargo test -- --nocapture
```

### 生成文档

```bash
# 生成 API 文档
cargo doc --no-deps --open
```

### 代码风格

项目遵循 Rust 标准代码风格：

```bash
# 格式化代码
cargo fmt

# 检查代码质量
cargo clippy
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
   ```

2. **广播权限**
   ```bash
   # 确保程序有网络广播权限
   # 在某些环境中可能需要管理员权限
   ```

3. **防火墙配置**
   ```bash
   # 确保 UDP 55555 端口开放
   sudo ufw allow 55555/udp
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

### v0.1.0 (2024-01-01)

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