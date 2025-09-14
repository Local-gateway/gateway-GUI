//! WDIC 网关库
//! 
//! 这是一个基于 QUIC 协议的本地网关实现，提供 P2P 网络发现和注册表管理功能。
//! 
//! # 主要功能
//! 
//! - 基于 QUIC (quiche) 的 WDIC 网络协议实现
//! - 本地网关注册表管理
//! - P2P 广播和发现机制
//! - 55555 端口服务监听
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

pub mod gateway;
pub mod registry;
pub mod protocol;
pub mod network;
pub mod udp_protocol;

pub use gateway::{Gateway, GatewayConfig};
pub use registry::{Registry, RegistryEntry};
pub use protocol::WdicProtocol;
pub use network::NetworkManager;
pub use udp_protocol::{UdpBroadcastManager, UdpToken, DirectoryIndex, UdpBroadcastEvent};