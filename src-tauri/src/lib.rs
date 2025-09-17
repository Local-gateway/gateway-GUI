mod tauri_commands;

use std::sync::{Arc, Mutex};
use wdic_gateway::tauri_api::GlobalGatewayState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            // Gateway API
            tauri_commands::start_gateway,
            tauri_commands::stop_gateway,
            tauri_commands::restart_gateway,
            tauri_commands::get_gateway_status,
            
            // Configuration API
            tauri_commands::get_gateway_config,
            tauri_commands::update_gateway_config,
            tauri_commands::validate_config,
            tauri_commands::reset_to_default_config,
            
            // Directory API
            tauri_commands::mount_directory,
            tauri_commands::unmount_directory,
            tauri_commands::get_mount_points,
            tauri_commands::list_directory,
            tauri_commands::create_file_transfer,
            tauri_commands::get_transfer_status,
            tauri_commands::cancel_transfer,
            
            // Network API
            tauri_commands::get_network_status,
            tauri_commands::start_p2p_discovery,
            tauri_commands::stop_p2p_discovery,
            tauri_commands::get_discovered_nodes,
            tauri_commands::connect_to_node,
            tauri_commands::disconnect_from_node,
            
            // Performance API
            tauri_commands::get_performance_report,
            tauri_commands::get_compression_stats,
            tauri_commands::get_cache_stats,
            tauri_commands::start_performance_benchmark,
            tauri_commands::get_benchmark_result,
            
            // Status API
            tauri_commands::get_system_info,
            tauri_commands::get_service_logs,
            tauri_commands::health_check,
            
            // Security API
            tauri_commands::get_security_config,
            tauri_commands::update_security_config,
            tauri_commands::generate_tls_certificate,
            tauri_commands::add_access_rule,
            tauri_commands::remove_access_rule,
            tauri_commands::get_access_rules,
            tauri_commands::validate_client_access,
            tauri_commands::get_active_sessions,
            tauri_commands::disconnect_session,
        ])
        .setup(|app| {
            // 初始化全局状态
            let global_state = tauri::async_runtime::block_on(async {
                GlobalGatewayState::new().await.expect("Failed to initialize global state")
            });
            
            app.manage(Arc::new(Mutex::new(global_state)));
            
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("启动 Tauri 应用程序时出错");
}
