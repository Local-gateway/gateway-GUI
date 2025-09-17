//! Tauri 命令函数模块
//!
//! 这个模块包含所有从主库重新导出的 Tauri 命令函数

use wdic_gateway::tauri_api::*;

// 重新导出所有命令函数
pub use wdic_gateway::tauri_api::{
    // Gateway API
    start_gateway,
    stop_gateway,
    restart_gateway,
    get_gateway_status,
    
    // Configuration API
    get_gateway_config,
    update_gateway_config,
    validate_config,
    reset_to_default_config,
    
    // Directory API
    mount_directory,
    unmount_directory,
    get_mount_points,
    list_directory,
    create_file_transfer,
    get_transfer_status,
    cancel_transfer,
    
    // Network API
    get_network_status,
    start_p2p_discovery,
    stop_p2p_discovery,
    get_discovered_nodes,
    connect_to_node,
    disconnect_from_node,
    
    // Performance API
    get_performance_report,
    get_compression_stats,
    get_cache_stats,
    start_performance_benchmark,
    get_benchmark_result,
    
    // Status API
    get_system_info,
    get_service_logs,
    health_check,
    
    // Security API
    get_security_config,
    update_security_config,
    generate_tls_certificate,
    add_access_rule,
    remove_access_rule,
    get_access_rules,
    validate_client_access,
    get_active_sessions,
    disconnect_session,
};
