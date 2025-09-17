//! WDIC 网关主程序
//!
//! 这是基于 Tauri 框架的 WDIC 网关应用程序主入口。
//! 集成了所有网关功能和 Tauri 后端 API。

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            // Gateway API
            wdic_gateway::tauri_api::start_gateway,
            wdic_gateway::tauri_api::stop_gateway,
            wdic_gateway::tauri_api::restart_gateway,
            wdic_gateway::tauri_api::get_gateway_status,
            
            // Configuration API
            wdic_gateway::tauri_api::get_gateway_config,
            wdic_gateway::tauri_api::update_gateway_config,
            wdic_gateway::tauri_api::validate_config,
            wdic_gateway::tauri_api::reset_to_default_config,
            
            // Directory API
            wdic_gateway::tauri_api::mount_directory,
            wdic_gateway::tauri_api::unmount_directory,
            wdic_gateway::tauri_api::get_mount_points,
            wdic_gateway::tauri_api::list_directory,
            wdic_gateway::tauri_api::create_file_transfer,
            wdic_gateway::tauri_api::get_transfer_status,
            wdic_gateway::tauri_api::cancel_transfer,
            
            // Network API
            wdic_gateway::tauri_api::get_network_status,
            wdic_gateway::tauri_api::start_p2p_discovery,
            wdic_gateway::tauri_api::stop_p2p_discovery,
            wdic_gateway::tauri_api::get_discovered_nodes,
            wdic_gateway::tauri_api::connect_to_node,
            wdic_gateway::tauri_api::disconnect_from_node,
            
            // Performance API
            wdic_gateway::tauri_api::get_performance_report,
            wdic_gateway::tauri_api::get_compression_stats,
            wdic_gateway::tauri_api::get_cache_stats,
            wdic_gateway::tauri_api::start_performance_benchmark,
            wdic_gateway::tauri_api::get_benchmark_result,
            
            // Status API
            wdic_gateway::tauri_api::get_system_info,
            wdic_gateway::tauri_api::get_service_logs,
            wdic_gateway::tauri_api::health_check,
            
            // Security API
            wdic_gateway::tauri_api::get_security_config,
            wdic_gateway::tauri_api::update_security_config,
            wdic_gateway::tauri_api::generate_tls_certificate,
            wdic_gateway::tauri_api::add_access_rule,
            wdic_gateway::tauri_api::remove_access_rule,
            wdic_gateway::tauri_api::get_access_rules,
            wdic_gateway::tauri_api::validate_client_access,
            wdic_gateway::tauri_api::get_active_sessions,
            wdic_gateway::tauri_api::disconnect_session,
        ])
        .setup(|_app| {
            // 初始化全局状态将在API调用时进行
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("启动 Tauri 应用程序时出错");
}

#[tokio::main]
async fn main() {
    // 初始化日志系统
    env_logger::init();

    // 检查是否在 Tauri 环境中运行
    if std::env::var("TAURI_ENV").is_ok() || std::env::args().any(|arg| arg.contains("tauri")) {
        // Tauri 模式：启动 GUI 应用
        println!("启动 WDIC 网关 GUI 应用程序");
        run();
    } else {
        // 命令行模式：运行传统网关
        println!("启动 WDIC 网关命令行版本");
        
        use wdic_gateway::Gateway;
        
        let gateway = Gateway::new("WDIC 网关".to_string()).await.expect("创建网关失败");
        
        println!("网关已启动，监听地址: {}", gateway.local_addr());
        println!("按 Ctrl+C 停止服务");
        
        // 运行网关
        if let Err(e) = gateway.run().await {
            eprintln!("网关运行错误: {e}");
            std::process::exit(1);
        }
    }
}
