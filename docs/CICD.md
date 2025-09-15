# CI/CD 工作流文档 (CI/CD Workflows Documentation)

本文档详细说明了 WDIC Gateway 项目的持续集成和持续部署 (CI/CD) 工作流程。

## 工作流概述

项目使用 GitHub Actions 实现完整的 CI/CD 流程，包含以下 4 个主要工作流：

1. **CI (持续集成)** - 代码质量检查和基础构建测试
2. **Mobile (移动平台构建)** - Android、iOS、HarmonyOS 平台构建
3. **Performance (性能测试)** - 自动化性能测试和监控
4. **Release (发布构建)** - 多平台发布版本构建

## CI 工作流 (.github/workflows/ci.yml)

### 触发条件
- 推送到 `main` 或 `develop` 分支
- 创建或更新 Pull Request

### 任务详情

#### 1. 代码质量检查 (test)
```yaml
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
    - 检出代码
    - 安装 Rust 工具链
    - 缓存依赖
    - 运行单元测试和集成测试
    - 运行 Clippy 代码检查
    - 检查代码格式化
```

**验证内容:**
- 所有单元测试通过
- 所有集成测试通过
- 代码符合 Clippy 规范
- 代码格式化正确

#### 2. Linux 平台构建 (build-linux)
```yaml
strategy:
  matrix:
    target:
      - x86_64-unknown-linux-gnu
      - aarch64-unknown-linux-gnu
      - armv7-unknown-linux-gnueabihf
```

**构建产物:**
- Linux x86_64 二进制文件
- Linux ARM64 二进制文件
- Linux ARMv7 二进制文件

#### 3. Windows 平台构建 (build-windows)
```yaml
strategy:
  matrix:
    target:
      - x86_64-pc-windows-msvc
```

**构建产物:**
- Windows x64 可执行文件

#### 4. macOS 平台构建 (build-macos)
```yaml
strategy:
  matrix:
    target:
      - x86_64-apple-darwin
      - aarch64-apple-darwin
```

**构建产物:**
- macOS Intel 二进制文件
- macOS Apple Silicon 二进制文件

## Mobile 工作流 (.github/workflows/mobile.yml)

### 触发条件
- 推送到 `main` 或 `develop` 分支
- 创建或更新 Pull Request  
- 手动触发 (`workflow_dispatch`)

### 任务详情

#### 1. Android 构建 (build-android)
```yaml
strategy:
  matrix:
    target:
      - aarch64-linux-android      # ARM64
      - armv7-linux-androideabi    # ARM32
      - i686-linux-android         # x86
      - x86_64-linux-android       # x86_64
```

**特殊设置:**
- 自动安装 Android NDK r25c
- 配置交叉编译环境变量
- 设置 Clang 编译器路径

#### 2. iOS 构建 (build-ios)
```yaml
strategy:
  matrix:
    target:
      - aarch64-apple-ios    # 设备
      - x86_64-apple-ios     # 模拟器
```

**运行环境:** macOS-latest (必需)

#### 3. HarmonyOS 构建 (build-harmonyos)
```yaml
strategy:
  matrix:
    target:
      - aarch64-linux-android      # 兼容 Android ARM64
      - armv7-linux-androideabi    # 兼容 Android ARM32
```

**说明:** HarmonyOS 使用 Android 兼容的构建目标

## Performance 工作流 (.github/workflows/performance.yml)

### 触发条件
- 推送到 `main` 或 `develop` 分支
- 创建或更新 Pull Request
- 每日定时执行 (UTC 2:00 AM)
- 手动触发

### 任务详情

#### 1. 基准测试 (benchmark)
```yaml
steps:
- 安装 Criterion 基准测试工具
- 运行性能基准测试
- 上传基准测试结果和图表
```

**测试内容:**
- 网络延迟基准测试
- 数据吞吐量测试
- 压缩性能测试
- 缓存性能测试

#### 2. 压力测试 (stress-test)
```yaml
strategy:
  matrix:
    test:
      - compression_stress
      - registry_stress
      - udp_broadcast_stress
      - performance_monitor_stress
      - gateway_concurrent_lifecycle
```

**测试场景:**
- 高并发压缩测试
- 注册表压力测试
- UDP 广播压力测试
- 性能监控压力测试
- 网关并发生命周期测试

#### 3. 内存分析 (memory-test)
```yaml
steps:
- 安装 Valgrind 内存检测工具
- 构建调试版本
- 运行内存泄漏检测
- 生成内存分析报告
```

#### 4. 性能对比 (performance-comparison)
```yaml
# 仅在 Pull Request 时运行
if: github.event_name == 'pull_request'
steps:
- 检出 PR 分支并运行基准测试
- 检出基础分支并运行基准测试
- 比较性能差异并生成报告
```

#### 5. 集成性能测试 (integration-performance)
```yaml
steps:
- 运行集成测试的性能测试用例
- 监控端到端性能指标
- 生成性能报告
```

#### 6. 网络性能测试 (network-performance)
```yaml
services:
  test-network:
    image: alpine:latest
steps:
- 运行网络相关的性能测试
- 测试 UDP 广播性能
- 测试 P2P 发现性能
```

## Release 工作流 (.github/workflows/release.yml)

### 触发条件
- 推送版本标签 (如 `v1.0.0`)
- 手动触发并指定版本标签

### 任务详情

#### 1. 创建发布 (create-release)
```yaml
steps:
- 基于标签创建 GitHub Release
- 设置发布标题和描述
- 返回上传 URL 供后续任务使用
```

#### 2. 构建和上传 (build-and-upload)
```yaml
strategy:
  matrix:
    include:
      # 14 种不同的平台/架构组合
      - os: ubuntu-latest
        target: x86_64-unknown-linux-gnu
        asset_name: wdic-gateway-linux-x86_64
      # ... 其他平台配置
```

**支持平台:**
- Linux: x86_64, aarch64, armv7
- Windows: x86_64
- macOS: x86_64, aarch64
- Android: aarch64, armv7, i686, x86_64
- iOS: aarch64, x86_64

#### 3. HarmonyOS 构建 (build-harmonyos)
```yaml
strategy:
  matrix:
    target:
      - aarch64-linux-android  # HarmonyOS ARM64
      - armv7-linux-androideabi # HarmonyOS ARM32
```

**特殊处理:**
- 使用专门的 HarmonyOS 资产命名
- 兼容 Android 构建环境
- 上传为单独的 HarmonyOS 版本

## 工作流依赖和缓存

### 依赖缓存策略
```yaml
- name: Cache dependencies
  uses: actions/cache@v3
  with:
    path: |
      ~/.cargo/registry
      ~/.cargo/git
      target
    key: ${{ runner.os }}-${{ matrix.target }}-cargo-${{ hashFiles('**/Cargo.lock') }}
```

**缓存内容:**
- Cargo 注册表
- Git 依赖
- 编译目标文件

### 缓存优化
- 基于操作系统和目标平台的分层缓存
- 基于 `Cargo.lock` 的缓存键
- 自动缓存失效和清理

## 工作流监控和通知

### 状态徽章
项目 README 包含所有工作流的状态徽章：

```markdown
[![CI](https://github.com/Local-gateway/gateway/actions/workflows/ci.yml/badge.svg)](https://github.com/Local-gateway/gateway/actions/workflows/ci.yml)
[![Mobile Builds](https://github.com/Local-gateway/gateway/actions/workflows/mobile.yml/badge.svg)](https://github.com/Local-gateway/gateway/actions/workflows/mobile.yml)
[![Performance Tests](https://github.com/Local-gateway/gateway/actions/workflows/performance.yml/badge.svg)](https://github.com/Local-gateway/gateway/actions/workflows/performance.yml)
[![Release](https://github.com/Local-gateway/gateway/actions/workflows/release.yml/badge.svg)](https://github.com/Local-gateway/gateway/actions/workflows/release.yml)
```

### 失败处理
- 自动重试临时失败
- 详细的错误日志输出
- 失败时的通知机制

## 本地开发和测试

### 本地运行 CI 测试
```bash
# 安装 act (本地 GitHub Actions 运行器)
curl https://raw.githubusercontent.com/nektos/act/master/install.sh | sudo bash

# 运行特定工作流
act -j test                    # 运行测试任务
act -j build-linux            # 运行 Linux 构建
act -j benchmark              # 运行基准测试

# 运行整个工作流
act -W .github/workflows/ci.yml
```

### 工作流调试
```bash
# 启用详细日志
act -v

# 运行特定事件
act push
act pull_request

# 使用自定义环境
act -s GITHUB_TOKEN=your_token
```

## 工作流配置最佳实践

### 1. 安全配置
- 使用 GitHub Secrets 管理敏感信息
- 最小权限原则
- 安全的依赖管理

### 2. 性能优化
- 有效的缓存策略
- 并行任务执行
- 条件任务执行

### 3. 可维护性
- 清晰的任务命名
- 详细的注释和文档
- 模块化的工作流设计

### 4. 监控和观察
- 完整的日志记录
- 性能指标收集
- 失败报告和分析

## 故障排除

### 常见问题

1. **构建失败**
   - 检查依赖版本兼容性
   - 验证交叉编译环境设置
   - 查看详细的构建日志

2. **测试失败**
   - 检查测试环境差异
   - 验证网络权限设置
   - 分析测试日志输出

3. **缓存问题**
   - 清理过期缓存
   - 验证缓存键配置
   - 检查缓存大小限制

4. **权限问题**
   - 验证 GitHub Token 权限
   - 检查仓库设置
   - 确认工作流权限配置

### 调试技巧

1. **启用调试日志**
   ```yaml
   - name: Enable debug logging
     run: echo "ACTIONS_STEP_DEBUG=true" >> $GITHUB_ENV
   ```

2. **保留失败的构建产物**
   ```yaml
   - name: Upload failed build logs
     if: failure()
     uses: actions/upload-artifact@v3
     with:
       name: build-logs
       path: target/debug/build/
   ```

3. **条件任务执行**
   ```yaml
   - name: Debug step
     if: runner.debug == 'true'
     run: echo "Debug information"
   ```

这套完整的 CI/CD 工作流确保了代码质量、跨平台兼容性和持续的性能监控，为项目的可靠性和可维护性提供了强有力的保障。