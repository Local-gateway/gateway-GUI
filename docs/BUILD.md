# 构建指南 (Build Guide)

本文档详细说明了如何为各种平台构建 WDIC Gateway。

## 支持的平台

### 桌面平台
- **Linux**: x86_64, aarch64, armv7
- **Windows**: x86_64
- **macOS**: x86_64, Apple Silicon (aarch64)

### 移动平台
- **Android**: armv7, aarch64, i686, x86_64
- **iOS**: aarch64 (设备), x86_64 (模拟器)
- **HarmonyOS**: aarch64, armv7

## 环境准备

### 基础要求
- Rust 1.89.0 或更高版本
- Git
- 网络访问权限

### 平台特定要求

#### Linux
```bash
# Ubuntu/Debian
sudo apt-get update
sudo apt-get install build-essential pkg-config libssl-dev

# CentOS/RHEL
sudo yum groupinstall "Development Tools"
sudo yum install pkgconfig openssl-devel

# Arch Linux
sudo pacman -S base-devel pkgconf openssl
```

#### Windows
- Visual Studio Build Tools 2019 或更新版本
- Windows SDK

#### macOS
```bash
xcode-select --install
brew install pkg-config openssl
```

## 交叉编译设置

### Android
```bash
# 1. 安装 Android NDK
# 下载并解压 Android NDK r25c 或更新版本
export ANDROID_NDK_ROOT=/path/to/android-ndk

# 2. 安装 Rust 目标
rustup target add aarch64-linux-android
rustup target add armv7-linux-androideabi  
rustup target add i686-linux-android
rustup target add x86_64-linux-android

# 3. 设置环境变量
export CC_aarch64_linux_android=$ANDROID_NDK_ROOT/toolchains/llvm/prebuilt/linux-x86_64/bin/aarch64-linux-android21-clang
export AR_aarch64_linux_android=$ANDROID_NDK_ROOT/toolchains/llvm/prebuilt/linux-x86_64/bin/llvm-ar

export CC_armv7_linux_androideabi=$ANDROID_NDK_ROOT/toolchains/llvm/prebuilt/linux-x86_64/bin/armv7a-linux-androideabi21-clang
export AR_armv7_linux_androideabi=$ANDROID_NDK_ROOT/toolchains/llvm/prebuilt/linux-x86_64/bin/llvm-ar

export CC_i686_linux_android=$ANDROID_NDK_ROOT/toolchains/llvm/prebuilt/linux-x86_64/bin/i686-linux-android21-clang
export AR_i686_linux_android=$ANDROID_NDK_ROOT/toolchains/llvm/prebuilt/linux-x86_64/bin/llvm-ar

export CC_x86_64_linux_android=$ANDROID_NDK_ROOT/toolchains/llvm/prebuilt/linux-x86_64/bin/x86_64-linux-android21-clang
export AR_x86_64_linux_android=$ANDROID_NDK_ROOT/toolchains/llvm/prebuilt/linux-x86_64/bin/llvm-ar

# 4. 构建
cargo build --target aarch64-linux-android --release
cargo build --target armv7-linux-androideabi --release
cargo build --target i686-linux-android --release
cargo build --target x86_64-linux-android --release
```

### iOS
```bash
# 1. 安装 Rust 目标 (仅限 macOS)
rustup target add aarch64-apple-ios
rustup target add x86_64-apple-ios

# 2. 构建
cargo build --target aarch64-apple-ios --release
cargo build --target x86_64-apple-ios --release
```

### Linux ARM
```bash
# 1. 安装交叉编译工具
sudo apt-get install gcc-aarch64-linux-gnu gcc-arm-linux-gnueabihf

# 2. 安装 Rust 目标
rustup target add aarch64-unknown-linux-gnu
rustup target add armv7-unknown-linux-gnueabihf

# 3. 构建
cargo build --target aarch64-unknown-linux-gnu --release
cargo build --target armv7-unknown-linux-gnueabihf --release
```

### Windows (在 Linux 上交叉编译)
```bash
# 1. 安装 MinGW
sudo apt-get install gcc-mingw-w64

# 2. 安装 Rust 目标
rustup target add x86_64-pc-windows-gnu

# 3. 构建
cargo build --target x86_64-pc-windows-gnu --release
```

### macOS (在 Linux 上交叉编译)
```bash
# 1. 安装 osxcross
git clone https://github.com/tpoechtrager/osxcross
cd osxcross
# 按照 osxcross 文档设置

# 2. 安装 Rust 目标
rustup target add x86_64-apple-darwin
rustup target add aarch64-apple-darwin

# 3. 设置环境变量
export CC_x86_64_apple_darwin=x86_64-apple-darwin20.4-clang
export CC_aarch64_apple_darwin=aarch64-apple-darwin20.4-clang

# 4. 构建
cargo build --target x86_64-apple-darwin --release
cargo build --target aarch64-apple-darwin --release
```

## 构建脚本

### 本地构建脚本
```bash
#!/bin/bash
# build-local.sh

set -e

echo "构建本地平台版本..."
cargo build --release

echo "运行测试..."
cargo test --release

echo "构建完成: target/release/wdic-gateway"
```

### 全平台构建脚本
```bash
#!/bin/bash
# build-all.sh

set -e

TARGETS=(
    "x86_64-unknown-linux-gnu"
    "aarch64-unknown-linux-gnu" 
    "armv7-unknown-linux-gnueabihf"
    "x86_64-pc-windows-msvc"
    "x86_64-apple-darwin"
    "aarch64-apple-darwin"
    "aarch64-linux-android"
    "armv7-linux-androideabi"
    "i686-linux-android"
    "x86_64-linux-android"
    "aarch64-apple-ios"
    "x86_64-apple-ios"
)

echo "安装所有目标..."
for target in "${TARGETS[@]}"; do
    rustup target add "$target"
done

echo "构建所有目标..."
for target in "${TARGETS[@]}"; do
    echo "构建 $target..."
    cargo build --target "$target" --release
done

echo "所有构建完成!"
```

## Docker 构建

### Dockerfile
```dockerfile
FROM rust:1.89 as builder

# 安装交叉编译依赖
RUN apt-get update && apt-get install -y \
    gcc-aarch64-linux-gnu \
    gcc-arm-linux-gnueabihf \
    gcc-mingw-w64 \
    && rm -rf /var/lib/apt/lists/*

# 安装目标
RUN rustup target add aarch64-unknown-linux-gnu
RUN rustup target add armv7-unknown-linux-gnueabihf

WORKDIR /usr/src/app
COPY . .

# 构建
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/src/app/target/release/wdic-gateway /usr/local/bin/wdic-gateway

EXPOSE 55555/udp 55556/udp
CMD ["wdic-gateway"]
```

### Docker 构建命令
```bash
# 构建镜像
docker build -t wdic-gateway .

# 运行容器
docker run -p 55555:55555/udp -p 55556:55556/udp wdic-gateway
```

## 优化构建

### 减小二进制大小
```toml
# Cargo.toml
[profile.release]
opt-level = "z"
lto = true
codegen-units = 1
panic = "abort"
strip = true
```

### 提高构建速度
```bash
# 使用 sccache
cargo install sccache
export RUSTC_WRAPPER=sccache

# 使用 mold 链接器 (Linux)
sudo apt-get install mold
export RUSTFLAGS="-C link-arg=-fuse-ld=mold"

# 并行构建
export CARGO_BUILD_JOBS=8
```

## 故障排除

### 常见构建错误

1. **链接错误**
```bash
# 确保安装了必要的系统库
sudo apt-get install build-essential pkg-config libssl-dev
```

2. **目标不可用**
```bash
# 列出所有可用目标
rustup target list

# 安装缺失的目标
rustup target add <target-name>
```

3. **Android NDK 路径错误**
```bash
# 确保 NDK 路径正确
echo $ANDROID_NDK_ROOT
ls $ANDROID_NDK_ROOT/toolchains/llvm/prebuilt/
```

4. **权限错误**
```bash
# 确保有写入权限
chmod +x build-scripts/*.sh
```

### 调试构建问题
```bash
# 详细输出
cargo build --verbose

# 显示链接器命令
cargo build --verbose 2>&1 | grep -i link

# 检查依赖
cargo tree
```

## 持续集成

项目使用 GitHub Actions 进行自动化构建，配置文件位于 `.github/workflows/` 目录。

### 本地 CI 测试
```bash
# 安装 act (本地运行 GitHub Actions)
curl https://raw.githubusercontent.com/nektos/act/master/install.sh | sudo bash

# 运行 CI
act -j test
act -j build-linux
```

这些构建指南确保您可以在任何支持的平台上成功构建 WDIC Gateway。如有问题，请参考故障排除部分或提交 issue。