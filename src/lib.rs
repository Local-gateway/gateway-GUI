//! WDIC 网关库
//!
//! 这是一个基于 QUIC 协议的本地网关实现，提供 P2P 网络发现和注册表管理功能。
//! 支持自动zstd压缩/解压缩和无锁并发优化。
//!
//! # 主要功能
//!
//! - 基于 QUIC (quiche) 的 WDIC 网络协议实现
//! - 本地网关注册表管理
//! - P2P 广播和发现机制
//! - 55555 端口服务监听
//! - 自动zstd压缩/解压缩传输优化
//! - 无锁并发数据结构
//!
//! # 使用示例
//!
//! ```no_run
//! use wdic_gateway::Gateway;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let gateway = Gateway::new("本地网关".to_string()).await?;
//!     gateway.run().await?;
//!     Ok(())
//! }
//! ```

pub mod cache;
pub mod compression;
pub mod gateway;
pub mod network;
pub mod performance;
pub mod protocol;
pub mod registry;
pub mod security;
pub mod tauri_api;
pub mod tauri_api_tests;
pub mod tls;
pub mod udp_protocol;

pub use cache::{CacheEntry, CacheMetadata, GatewayCache};
pub use compression::{
    CompressionConfig, CompressionFlag, CompressionManager, CompressionStats,
    CompressionStatsSnapshot,
};
pub use gateway::{Gateway, GatewayConfig};
pub use network::NetworkManager;
pub use performance::{
    BenchmarkResult, PerformanceMonitor, PerformanceReport, PerformanceTestSuite,
};
pub use protocol::WdicProtocol;
pub use registry::{Registry, RegistryEntry};
pub use tauri_api::{
    GlobalGatewayState, GatewayStatus, NetworkStatus, MountPoint, FileTransferTask,
    SecurityConfig, AccessRule, SystemInfo, HealthStatus, LogEntry, CacheStats,
    BenchmarkResult as TauriBenchmarkResult, BenchmarkStatus,
    DirectoryEntry as TauriDirectoryEntry, DiscoveredNode, CertificateInfo, GeneratedCertificate,
    ActiveSession, TransferStatus, NetworkInterface,
};
pub use security::{PathValidator, SecureFileReader, SearchResultFilter};
pub use tls::{MtlsConfig, TlsManager, TlsVersion, VerifyMode};
pub use udp_protocol::{
    DirectoryEntry, DirectoryIndex, UdpBroadcastEvent, UdpBroadcastManager, UdpToken,
};

// Add integration test to verify BoringSSL works properly
#[cfg(test)]
mod boringssl_integration {
    #[test]
    fn test_quiche_boringssl_integration() {
        // Test that quiche can be initialized (which requires BoringSSL)
        let mut config = quiche::Config::new(quiche::PROTOCOL_VERSION).unwrap();

        // Set some basic configuration to verify the library works
        config.set_application_protos(&[b"test"]).unwrap();
        config.set_max_idle_timeout(5000);
        config.set_max_recv_udp_payload_size(1350);
        config.set_max_send_udp_payload_size(1350);
        config.set_initial_max_data(10_000_000);
        config.set_initial_max_stream_data_bidi_local(1_000_000);
        config.set_initial_max_stream_data_bidi_remote(1_000_000);
        config.set_initial_max_streams_bidi(100);
        config.set_initial_max_streams_uni(100);
        config.set_disable_active_migration(true);

        // This would fail if BoringSSL wasn't properly linked
        println!("✅ quiche configuration created successfully!");
        println!("✅ BoringSSL integration is working correctly!");
        println!("✅ QUIC protocol version: {}", quiche::PROTOCOL_VERSION);
    }
}
