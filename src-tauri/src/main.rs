//! WDIC 网关 Tauri 应用程序主入口

#[tokio::main]
async fn main() {
    // 初始化日志系统
    env_logger::init();

    // 启动 Tauri 应用
    wdic_gateway_tauri::run();
}
