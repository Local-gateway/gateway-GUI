# 项目概览 (Project Overview)

## WDIC Gateway - 跨平台本地网关

WDIC Gateway 是一个高性能、跨平台的本地网关实现，基于 QUIC + UDP 双协议设计，支持 P2P 网络发现、文件传输、智能搜索和实时通信功能。

## 🚀 核心特性

### 网络协议
- **QUIC 协议**: 安全的点对点通信
- **UDP 广播**: 快速网络发现和消息传递
- **双端口设计**: 55555 (QUIC) + 55556 (UDP)
- **IPv4/IPv6 双栈**: 全面的网络协议支持

### 安全与性能
- **TLS 1.3 mTLS**: 双向认证和端到端加密
- **zstd 压缩**: 自动数据压缩，节省带宽
- **智能缓存**: 高效的数据缓存机制
- **性能监控**: 实时性能指标和基准测试

### 功能特性
- **目录挂载**: 自动索引生成和文件搜索
- **文件传输**: 安全的 P2P 文件传输
- **信息广播**: 实时消息传递
- **网络发现**: 自动发现网络中的其他网关

## 🏗️ 跨平台支持

### 桌面平台
- **Linux**: x86_64, aarch64, armv7
- **Windows**: x86_64
- **macOS**: x86_64, Apple Silicon (aarch64)

### 移动平台
- **Android**: armv7, aarch64, i686, x86_64
- **iOS**: aarch64 (设备), x86_64 (模拟器)
- **HarmonyOS**: aarch64, armv7

## 🤖 自动化系统

### CI/CD 工作流
- **持续集成**: 代码质量检查和测试
- **移动构建**: 自动化移动平台编译
- **性能测试**: 自动化性能监控和回归测试
- **发布构建**: 多平台发布版本自动化

### 测试覆盖
- **单元测试**: 69+ 个单元测试用例
- **集成测试**: 9 个集成测试场景
- **压力测试**: 5 个压力测试场景
- **性能基准**: 完整的基准测试套件

## 📊 性能指标

### 网络性能
- **低延迟**: QUIC 协议优化的网络通信
- **高吞吐量**: 支持大文件和并发传输
- **智能广播**: 减少网络流量的广播策略

### 系统性能
- **内存效率**: 优化的数据结构和内存管理
- **CPU 友好**: 异步 I/O 和多线程优化
- **存储优化**: 压缩缓存和智能索引

## 📖 文档体系

### 用户文档
- **README.md**: 项目介绍和快速开始
- **docs/BUILD.md**: 详细的构建指南
- **docs/DEPLOYMENT.md**: 全面的部署指南

### 开发文档
- **docs/API.md**: 完整的 API 文档
- **docs/PERFORMANCE.md**: 性能测试指南
- **docs/CICD.md**: CI/CD 工作流文档

### 技术文档
- **IMPLEMENTATION_SUMMARY.md**: 实现总结
- **PERFORMANCE_OPTIMIZATION.md**: 性能优化文档

## 🔧 开发工具

### 构建工具
- **Cargo**: Rust 包管理和构建
- **Cross**: 跨平台编译支持
- **Docker**: 容器化构建环境

### 测试工具
- **Criterion**: 性能基准测试
- **Tokio-test**: 异步测试框架
- **Valgrind**: 内存泄漏检测

### CI/CD 工具
- **GitHub Actions**: 自动化工作流
- **缓存系统**: 构建依赖优化
- **Artifact 管理**: 构建产物存储

## 🎯 使用场景

### 个人用户
- 家庭网络文件共享
- 设备间文件同步
- 本地网络通信

### 企业用户
- 内网文件分发
- 办公室网络发现
- 安全文档传输

### 开发者
- P2P 应用开发
- 网络协议研究
- 分布式系统测试

## 🚀 快速开始

```bash
# 下载预编译版本
wget https://github.com/Local-gateway/gateway/releases/latest/download/wdic-gateway-linux-x86_64

# 或从源码构建
git clone https://github.com/Local-gateway/gateway.git
cd gateway
cargo build --release

# 运行网关
./target/release/wdic-gateway
```

## 📈 项目状态

[![CI](https://github.com/Local-gateway/gateway/actions/workflows/ci.yml/badge.svg)](https://github.com/Local-gateway/gateway/actions/workflows/ci.yml)
[![Mobile Builds](https://github.com/Local-gateway/gateway/actions/workflows/mobile.yml/badge.svg)](https://github.com/Local-gateway/gateway/actions/workflows/mobile.yml)
[![Performance Tests](https://github.com/Local-gateway/gateway/actions/workflows/performance.yml/badge.svg)](https://github.com/Local-gateway/gateway/actions/workflows/performance.yml)
[![Release](https://github.com/Local-gateway/gateway/actions/workflows/release.yml/badge.svg)](https://github.com/Local-gateway/gateway/actions/workflows/release.yml)

- **稳定版本**: v0.2.0
- **开发版本**: v0.3.0 (进行中)
- **测试覆盖**: 78+ 测试用例
- **平台支持**: 14 种平台/架构

## 🤝 贡献指南

1. Fork 项目仓库
2. 创建功能分支
3. 实现功能并添加测试
4. 确保所有测试通过
5. 提交 Pull Request

## 📄 许可证

本项目采用 MIT 许可证开源。

## 🔗 相关链接

- **项目主页**: https://github.com/Local-gateway/gateway
- **API 文档**: https://docs.rs/wdic-gateway
- **问题反馈**: https://github.com/Local-gateway/gateway/issues
- **发布页面**: https://github.com/Local-gateway/gateway/releases