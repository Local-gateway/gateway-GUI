# 部署指南 (Deployment Guide)

本文档详细说明了如何在各种环境中部署 WDIC Gateway。

## 部署方式概览

WDIC Gateway 支持多种部署方式：

1. **单机部署** - 本地运行单个网关实例
2. **集群部署** - 多机器运行多个网关实例
3. **容器化部署** - 使用 Docker 容器化部署
4. **云原生部署** - 使用 Kubernetes 部署
5. **移动设备部署** - 在 Android/iOS 设备上运行

## 系统要求

### 最低配置
- **CPU**: 单核 1GHz+
- **内存**: 512MB RAM
- **存储**: 100MB 可用空间
- **网络**: UDP 端口 55555-55556 可访问

### 推荐配置
- **CPU**: 双核 2GHz+
- **内存**: 2GB RAM
- **存储**: 1GB 可用空间
- **网络**: 千兆网络连接

### 网络要求
- UDP 端口 55555 (QUIC 协议)
- UDP 端口 55556 (UDP 广播)
- 支持广播和组播
- 防火墙允许相关端口通信

## 平台特定部署

### Linux 部署

#### Ubuntu/Debian
```bash
# 1. 下载预编译二进制文件
wget https://github.com/Local-gateway/gateway/releases/latest/download/wdic-gateway-linux-x86_64
chmod +x wdic-gateway-linux-x86_64

# 2. 创建系统用户
sudo useradd --system --no-create-home --shell /bin/false wdic-gateway

# 3. 创建工作目录
sudo mkdir -p /opt/wdic-gateway
sudo mkdir -p /var/lib/wdic-gateway
sudo mkdir -p /var/log/wdic-gateway

# 4. 安装二进制文件
sudo mv wdic-gateway-linux-x86_64 /opt/wdic-gateway/wdic-gateway
sudo chown wdic-gateway:wdic-gateway /opt/wdic-gateway/wdic-gateway
sudo chown -R wdic-gateway:wdic-gateway /var/lib/wdic-gateway
sudo chown -R wdic-gateway:wdic-gateway /var/log/wdic-gateway

# 5. 创建 systemd 服务文件
sudo tee /etc/systemd/system/wdic-gateway.service > /dev/null <<EOF
[Unit]
Description=WDIC Gateway
After=network.target

[Service]
Type=simple
User=wdic-gateway
Group=wdic-gateway
ExecStart=/opt/wdic-gateway/wdic-gateway
Restart=always
RestartSec=5
Environment=RUST_LOG=info
WorkingDirectory=/var/lib/wdic-gateway
StandardOutput=append:/var/log/wdic-gateway/stdout.log
StandardError=append:/var/log/wdic-gateway/stderr.log

[Install]
WantedBy=multi-user.target
EOF

# 6. 启动服务
sudo systemctl daemon-reload
sudo systemctl enable wdic-gateway
sudo systemctl start wdic-gateway
```

#### CentOS/RHEL
```bash
# 1. 下载二进制文件
curl -L -o wdic-gateway-linux-x86_64 \
  https://github.com/Local-gateway/gateway/releases/latest/download/wdic-gateway-linux-x86_64
chmod +x wdic-gateway-linux-x86_64

# 2. 防火墙配置
sudo firewall-cmd --permanent --add-port=55555/udp
sudo firewall-cmd --permanent --add-port=55556/udp
sudo firewall-cmd --reload

# 3. SELinux 配置 (如果启用)
sudo setsebool -P nis_enabled 1
sudo semanage port -a -t generic_port_t -p udp 55555
sudo semanage port -a -t generic_port_t -p udp 55556

# 其余步骤与 Ubuntu 相同
```

### Windows 部署

#### Windows 服务安装
```powershell
# 1. 下载二进制文件
Invoke-WebRequest -Uri "https://github.com/Local-gateway/gateway/releases/latest/download/wdic-gateway-windows-x86_64.exe" -OutFile "wdic-gateway.exe"

# 2. 创建服务目录
New-Item -Path "C:\Program Files\WDIC Gateway" -ItemType Directory -Force
Move-Item -Path "wdic-gateway.exe" -Destination "C:\Program Files\WDIC Gateway\"

# 3. 防火墙配置
New-NetFirewallRule -DisplayName "WDIC Gateway UDP 55555" -Direction Inbound -Protocol UDP -LocalPort 55555 -Action Allow
New-NetFirewallRule -DisplayName "WDIC Gateway UDP 55556" -Direction Inbound -Protocol UDP -LocalPort 55556 -Action Allow

# 4. 使用 NSSM 创建 Windows 服务
# 下载 NSSM: https://nssm.cc/download
.\nssm.exe install "WDIC Gateway" "C:\Program Files\WDIC Gateway\wdic-gateway.exe"
.\nssm.exe set "WDIC Gateway" Description "WDIC Gateway P2P Network Service"
.\nssm.exe set "WDIC Gateway" Start SERVICE_AUTO_START

# 5. 启动服务
Start-Service "WDIC Gateway"
```

### macOS 部署

#### 使用 Homebrew
```bash
# 1. 安装 Homebrew (如果尚未安装)
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"

# 2. 下载二进制文件
curl -L -o wdic-gateway-macos-aarch64 \
  https://github.com/Local-gateway/gateway/releases/latest/download/wdic-gateway-macos-aarch64
chmod +x wdic-gateway-macos-aarch64

# 3. 安装到系统路径
sudo mv wdic-gateway-macos-aarch64 /usr/local/bin/wdic-gateway

# 4. 创建 launchd 服务
sudo tee /Library/LaunchDaemons/com.wdic.gateway.plist > /dev/null <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.wdic.gateway</string>
    <key>ProgramArguments</key>
    <array>
        <string>/usr/local/bin/wdic-gateway</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>/var/log/wdic-gateway.log</string>
    <key>StandardErrorPath</key>
    <string>/var/log/wdic-gateway.error.log</string>
</dict>
</plist>
EOF

# 5. 加载并启动服务
sudo launchctl load /Library/LaunchDaemons/com.wdic.gateway.plist
sudo launchctl start com.wdic.gateway
```

## 容器化部署

### Docker 部署

#### 使用预构建镜像
```bash
# 1. 拉取镜像
docker pull ghcr.io/local-gateway/gateway:latest

# 2. 运行容器
docker run -d \
  --name wdic-gateway \
  --restart unless-stopped \
  -p 55555:55555/udp \
  -p 55556:55556/udp \
  -v wdic-gateway-data:/var/lib/wdic-gateway \
  -e RUST_LOG=info \
  ghcr.io/local-gateway/gateway:latest
```

#### 自定义 Dockerfile
```dockerfile
FROM debian:bookworm-slim

# 安装运行时依赖
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# 创建用户
RUN useradd --system --no-create-home --shell /bin/false wdic-gateway

# 复制二进制文件
COPY target/release/wdic-gateway /usr/local/bin/wdic-gateway
RUN chmod +x /usr/local/bin/wdic-gateway

# 创建数据目录
RUN mkdir -p /var/lib/wdic-gateway && \
    chown wdic-gateway:wdic-gateway /var/lib/wdic-gateway

# 暴露端口
EXPOSE 55555/udp 55556/udp

# 切换用户
USER wdic-gateway

# 设置工作目录
WORKDIR /var/lib/wdic-gateway

# 启动命令
CMD ["wdic-gateway"]
```

#### Docker Compose 部署
```yaml
version: '3.8'

services:
  wdic-gateway:
    image: ghcr.io/local-gateway/gateway:latest
    container_name: wdic-gateway
    restart: unless-stopped
    ports:
      - "55555:55555/udp"
      - "55556:55556/udp"
    volumes:
      - wdic-gateway-data:/var/lib/wdic-gateway
      - ./logs:/var/log/wdic-gateway
    environment:
      - RUST_LOG=info
      - GATEWAY_NAME=Docker Gateway
    networks:
      - wdic-network

volumes:
  wdic-gateway-data:

networks:
  wdic-network:
    driver: bridge
```

### Kubernetes 部署

#### Deployment 配置
```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: wdic-gateway
  labels:
    app: wdic-gateway
spec:
  replicas: 3
  selector:
    matchLabels:
      app: wdic-gateway
  template:
    metadata:
      labels:
        app: wdic-gateway
    spec:
      containers:
      - name: wdic-gateway
        image: ghcr.io/local-gateway/gateway:latest
        ports:
        - containerPort: 55555
          protocol: UDP
        - containerPort: 55556
          protocol: UDP
        env:
        - name: RUST_LOG
          value: "info"
        - name: GATEWAY_NAME
          valueFrom:
            fieldRef:
              fieldPath: metadata.name
        resources:
          requests:
            memory: "256Mi"
            cpu: "100m"
          limits:
            memory: "512Mi"
            cpu: "500m"
        volumeMounts:
        - name: data
          mountPath: /var/lib/wdic-gateway
      volumes:
      - name: data
        emptyDir: {}
---
apiVersion: v1
kind: Service
metadata:
  name: wdic-gateway-service
spec:
  selector:
    app: wdic-gateway
  ports:
  - name: quic
    port: 55555
    protocol: UDP
    targetPort: 55555
  - name: udp-broadcast
    port: 55556
    protocol: UDP
    targetPort: 55556
  type: LoadBalancer
```

#### ConfigMap 配置
```yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: wdic-gateway-config
data:
  gateway.toml: |
    [gateway]
    name = "Kubernetes Gateway"
    port = 55555
    broadcast_interval = 30
    heartbeat_interval = 60
    connection_timeout = 300
    registry_cleanup_interval = 120
```

## 移动设备部署

### Android 部署

#### APK 打包 (需要额外的 Android 包装器)
```bash
# 1. 构建 Android 版本
cargo build --target aarch64-linux-android --release

# 2. 使用 Android Studio 创建包装器应用
# 或使用 Tauri/Flutter 等框架
```

#### 在 Termux 中运行
```bash
# 1. 安装 Termux
# 2. 在 Termux 中下载二进制文件
pkg update && pkg install wget
wget https://github.com/Local-gateway/gateway/releases/latest/download/wdic-gateway-android-aarch64

# 3. 运行
chmod +x wdic-gateway-android-aarch64
./wdic-gateway-android-aarch64
```

### iOS 部署

iOS 部署需要通过 App Store 或企业分发，需要额外的 iOS 应用包装器。

## 配置管理

### 环境变量配置
```bash
# 基础配置
export RUST_LOG=info                    # 日志级别
export GATEWAY_NAME="My Gateway"        # 网关名称
export GATEWAY_PORT=55555               # 端口号

# 高级配置
export BROADCAST_INTERVAL=30            # 广播间隔(秒)
export HEARTBEAT_INTERVAL=60            # 心跳间隔(秒)
export CONNECTION_TIMEOUT=300           # 连接超时(秒)
export REGISTRY_CLEANUP_INTERVAL=120    # 注册表清理间隔(秒)
```

### 配置文件
```toml
# /etc/wdic-gateway/config.toml
[gateway]
name = "Production Gateway"
port = 55555
broadcast_interval = 30
heartbeat_interval = 60
connection_timeout = 300
registry_cleanup_interval = 120

[logging]
level = "info"
file = "/var/log/wdic-gateway/app.log"

[network]
bind_address = "0.0.0.0"
broadcast_addresses = ["255.255.255.255:55556"]
```

## 监控和日志

### 日志配置
```bash
# 启用详细日志
export RUST_LOG=wdic_gateway=debug,info

# 日志文件轮转
sudo tee /etc/logrotate.d/wdic-gateway > /dev/null <<EOF
/var/log/wdic-gateway/*.log {
    daily
    missingok
    rotate 30
    compress
    delaycompress
    notifempty
    create 644 wdic-gateway wdic-gateway
    postrotate
        systemctl reload wdic-gateway
    endscript
}
EOF
```

### 监控集成

#### Prometheus 监控
```yaml
# prometheus.yml
scrape_configs:
  - job_name: 'wdic-gateway'
    static_configs:
      - targets: ['localhost:8080']  # 需要添加 metrics 端点
```

#### 健康检查
```bash
#!/bin/bash
# /usr/local/bin/wdic-gateway-health-check.sh

# 检查进程是否运行
if ! pgrep -f wdic-gateway > /dev/null; then
    echo "CRITICAL: WDIC Gateway process not running"
    exit 2
fi

# 检查端口是否监听
if ! netstat -ulpn | grep -q ":55555 "; then
    echo "CRITICAL: WDIC Gateway not listening on port 55555"
    exit 2
fi

echo "OK: WDIC Gateway is running normally"
exit 0
```

## 性能调优

### 系统级优化
```bash
# 增加文件描述符限制
echo 'wdic-gateway soft nofile 65536' >> /etc/security/limits.conf
echo 'wdic-gateway hard nofile 65536' >> /etc/security/limits.conf

# 调整网络缓冲区
echo 'net.core.rmem_max = 134217728' >> /etc/sysctl.conf
echo 'net.core.wmem_max = 134217728' >> /etc/sysctl.conf
sysctl -p
```

### 应用级优化
```bash
# 设置 CPU 亲和性
taskset -c 0,1 /opt/wdic-gateway/wdic-gateway

# 设置进程优先级
nice -n -10 /opt/wdic-gateway/wdic-gateway
```

## 故障排除

### 常见问题

1. **端口占用**
```bash
# 检查端口占用
ss -ulpn | grep 55555
lsof -i :55555
```

2. **权限问题**
```bash
# 检查文件权限
ls -la /opt/wdic-gateway/
ls -la /var/lib/wdic-gateway/

# 修复权限
sudo chown -R wdic-gateway:wdic-gateway /var/lib/wdic-gateway/
```

3. **网络连接问题**
```bash
# 测试网络连通性
nc -u 127.0.0.1 55555

# 检查防火墙
sudo iptables -L -n | grep 55555
```

### 调试模式
```bash
# 启用调试日志
RUST_LOG=debug /opt/wdic-gateway/wdic-gateway

# 使用 strace 跟踪系统调用
strace -f -e trace=network /opt/wdic-gateway/wdic-gateway
```

这套部署指南涵盖了各种环境下的部署方案，确保 WDIC Gateway 能够在任何环境中稳定运行。