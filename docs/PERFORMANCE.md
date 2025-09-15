# 性能测试指南 (Performance Testing Guide)

本文档详细说明了 WDIC Gateway 的性能测试系统，包括基准测试、压力测试和性能监控。

## 性能测试概述

WDIC Gateway 包含多层次的性能测试系统：

1. **基准测试** - 使用 Criterion 进行精确的性能测量
2. **压力测试** - 高负载和并发场景测试
3. **集成性能测试** - 端到端性能验证
4. **内存分析** - 内存使用和泄漏检测
5. **网络性能测试** - 网络通信效率测试

## 基准测试

### 运行基准测试

```bash
# 运行所有基准测试
cargo bench

# 运行特定基准测试
cargo bench --bench performance_benchmarks

# 生成 HTML 报告
cargo bench --bench performance_benchmarks -- --output-format html

# 保存基准测试结果
cargo bench --bench performance_benchmarks > benchmark_results.txt
```

### 基准测试套件

项目包含以下基准测试：

#### 1. 延迟测试
```rust
// 测试网络延迟
#[bench]
fn bench_network_latency(b: &mut Bencher) {
    // 测试 UDP 广播延迟
    // 测试 QUIC 连接延迟
    // 测试 P2P 发现延迟
}
```

#### 2. 吞吐量测试
```rust
// 测试数据传输吞吐量
#[bench]
fn bench_data_throughput(b: &mut Bencher) {
    // 测试文件传输速度
    // 测试消息广播速度
    // 测试并发连接处理能力
}
```

#### 3. 压缩性能测试
```rust
// 测试数据压缩性能
#[bench]
fn bench_compression_performance(b: &mut Bencher) {
    // 测试不同大小数据的压缩速度
    // 测试压缩率 vs 速度权衡
    // 测试解压缩性能
}
```

#### 4. 缓存性能测试
```rust
// 测试缓存系统性能
#[bench]
fn bench_cache_performance(b: &mut Bencher) {
    // 测试缓存命中率
    // 测试缓存查询速度
    // 测试缓存清理性能
}
```

### 基准测试结果分析

基准测试生成详细的性能报告，包括：

- **平均执行时间**
- **标准偏差**
- **置信区间**
- **吞吐量指标**
- **性能回归检测**

## 压力测试

### 运行压力测试

```bash
# 运行所有压力测试
cargo test --test stress_tests --release

# 运行特定压力测试
cargo test --test stress_tests test_compression_stress --release -- --nocapture
cargo test --test stress_tests test_registry_stress --release -- --nocapture
cargo test --test stress_tests test_udp_broadcast_stress --release -- --nocapture
cargo test --test stress_tests test_performance_monitor_stress --release -- --nocapture
cargo test --test stress_tests test_gateway_concurrent_lifecycle --release -- --nocapture
```

### 压力测试场景

#### 1. 压缩压力测试
- 并发压缩大量数据
- 测试内存使用限制
- 验证压缩质量稳定性

#### 2. 注册表压力测试
- 大量并发注册和查询
- 高频率更新操作
- 内存和性能稳定性验证

#### 3. UDP 广播压力测试
- 高频率消息广播
- 大量并发连接
- 网络拥塞处理

#### 4. 网关生命周期压力测试
- 频繁启动和停止
- 并发多实例运行
- 资源清理验证

## 内存分析

### Valgrind 内存检查

```bash
# 构建调试版本
cargo build

# 运行内存检查
valgrind --tool=memcheck \
         --leak-check=full \
         --show-leak-kinds=all \
         --track-origins=yes \
         --verbose \
         target/debug/wdic-gateway --help

# 检查特定功能的内存使用
valgrind --tool=memcheck \
         --leak-check=full \
         target/debug/wdic-gateway
```

### 内存使用监控

```rust
// 内存使用监控示例
use sysinfo::{System, SystemExt};

fn monitor_memory_usage() {
    let mut system = System::new_all();
    system.refresh_all();
    
    let process = system.process(std::process::id()).unwrap();
    println!("内存使用: {} KB", process.memory());
    println!("虚拟内存: {} KB", process.virtual_memory());
}
```

### 内存泄漏检测

项目包含自动化内存泄漏检测：

```bash
# 运行内存泄漏测试
cargo test test_memory_stress --release -- --nocapture
```

## 网络性能测试

### UDP 广播性能

```bash
# 测试 UDP 广播性能
cargo test --test integration_tests test_udp_broadcast_functionality --release -- --nocapture
```

### P2P 发现性能

```bash
# 测试 P2P 发现性能
cargo test --test integration_tests test_p2p_discovery --release -- --nocapture
```

### QUIC 连接性能

```bash
# 测试 QUIC 连接性能
cargo test --test integration_tests test_gateway_lifecycle --release -- --nocapture
```

## 性能监控

### 实时性能监控

WDIC Gateway 内置实时性能监控系统：

```rust
// 创建性能监控器
let monitor = PerformanceMonitor::new();

// 记录延迟
monitor.record_latency("udp_broadcast", latency);

// 记录吞吐量
monitor.record_throughput("file_transfer", bytes_per_second);

// 生成性能报告
let report = monitor.generate_report();
```

### 性能指标

监控的关键性能指标包括：

1. **网络延迟**
   - UDP 广播延迟
   - QUIC 连接延迟
   - P2P 发现时间

2. **吞吐量**
   - 数据传输速率
   - 消息处理速率
   - 并发连接数

3. **资源使用**
   - CPU 使用率
   - 内存使用量
   - 网络带宽使用

4. **错误率**
   - 连接失败率
   - 超时比率
   - 重传次数

## 性能分析工具

### Perf (Linux)

```bash
# 安装 perf
sudo apt-get install linux-tools-generic

# 记录性能数据
perf record --call-graph dwarf target/release/wdic-gateway

# 查看性能报告
perf report

# 生成火焰图
perf script | ~/FlameGraph/stackcollapse-perf.pl | ~/FlameGraph/flamegraph.pl > perf.svg
```

### Instruments (macOS)

```bash
# 使用 Instruments 进行性能分析
xcrun xctrace record --template "Time Profiler" --launch target/release/wdic-gateway
```

### Flamegraph

```bash
# 安装 flamegraph
cargo install flamegraph

# 生成火焰图
cargo flamegraph --bin wdic-gateway

# 生成基准测试火焰图
cargo flamegraph --bench performance_benchmarks
```

## 持续性能监控

### GitHub Actions 集成

项目通过 GitHub Actions 实现持续性能监控：

```yaml
# .github/workflows/performance.yml
- name: Run Benchmarks
  run: |
    cargo bench --bench performance_benchmarks > benchmark_results.txt
    cat benchmark_results.txt

- name: Performance Regression Check
  run: |
    # 比较当前结果与基线
    python scripts/check_performance_regression.py
```

### 性能回归检测

自动检测性能回归：

```python
# scripts/check_performance_regression.py
def check_performance_regression(current_results, baseline_results):
    """检查性能回归"""
    for test_name, current_time in current_results.items():
        baseline_time = baseline_results.get(test_name)
        if baseline_time:
            regression = (current_time - baseline_time) / baseline_time
            if regression > 0.1:  # 10% 性能下降
                print(f"性能回归检测: {test_name} 下降 {regression*100:.1f}%")
                return False
    return True
```

## 性能优化建议

### 编译优化

```toml
# Cargo.toml
[profile.release]
opt-level = 3
lto = true
codegen-units = 1
```

### 运行时优化

```bash
# 设置环境变量优化性能
export RUST_BACKTRACE=0
export TOKIO_WORKER_THREADS=8

# 使用性能 CPU 调度器
sudo cpupower frequency-set --governor performance
```

### 网络优化

```bash
# 调整网络缓冲区大小
echo 'net.core.rmem_max = 134217728' | sudo tee -a /etc/sysctl.conf
echo 'net.core.wmem_max = 134217728' | sudo tee -a /etc/sysctl.conf
sudo sysctl -p
```

## 性能测试最佳实践

1. **一致的测试环境** - 使用相同的硬件和网络条件
2. **预热测试** - 运行预热轮次以稳定性能
3. **多次运行** - 进行多次测试取平均值
4. **基线比较** - 与之前版本进行性能对比
5. **资源监控** - 同时监控 CPU、内存、网络使用情况

## 性能问题排查

### 识别瓶颈

1. **CPU 瓶颈**
   ```bash
   # 检查 CPU 使用率
   top -p $(pgrep wdic-gateway)
   ```

2. **内存瓶颈**
   ```bash
   # 检查内存使用
   cat /proc/$(pgrep wdic-gateway)/status | grep VmRSS
   ```

3. **网络瓶颈**
   ```bash
   # 监控网络使用
   iftop -i eth0
   ```

### 性能调优

1. **调整线程池大小**
2. **优化缓存策略**
3. **减少内存分配**
4. **优化网络协议参数**

通过这套完整的性能测试系统，可以确保 WDIC Gateway 在各种负载条件下都能保持优异的性能表现。