//! WDIC 协议模块
//! 
//! 实现基于 QUIC 的 WDIC (Web Dynamic Inter-Connection) 网络协议。

use serde::{Deserialize, Serialize};
use crate::registry::RegistryEntry;
use std::net::SocketAddr;
use uuid::Uuid;

/// WDIC 协议消息类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum WdicMessage {
    /// 广播消息 - 宣告自己的存在
    Broadcast {
        /// 发送者信息
        sender: RegistryEntry,
    },
    /// 广播响应 - 返回已知的网关列表
    BroadcastResponse {
        /// 响应者信息
        sender: RegistryEntry,
        /// 已知网关列表（不包括请求者）
        gateways: Vec<RegistryEntry>,
    },
    /// 心跳消息 - 保持连接活跃
    Heartbeat {
        /// 发送者 ID
        sender_id: Uuid,
        /// 时间戳
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    /// 心跳响应
    HeartbeatResponse {
        /// 响应者 ID
        sender_id: Uuid,
        /// 时间戳
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    /// 网关注册请求
    RegisterRequest {
        /// 请求注册的网关信息
        gateway: RegistryEntry,
    },
    /// 网关注册响应
    RegisterResponse {
        /// 是否成功注册
        success: bool,
        /// 响应消息
        message: String,
        /// 已知网关列表
        gateways: Vec<RegistryEntry>,
    },
    /// 网关注销请求
    UnregisterRequest {
        /// 要注销的网关 ID
        gateway_id: Uuid,
    },
    /// 网关注销响应
    UnregisterResponse {
        /// 是否成功注销
        success: bool,
        /// 响应消息
        message: String,
    },
    /// 查询网关列表
    QueryGateways {
        /// 查询者 ID
        requester_id: Uuid,
    },
    /// 查询响应
    QueryResponse {
        /// 响应者 ID
        sender_id: Uuid,
        /// 网关列表
        gateways: Vec<RegistryEntry>,
    },
    /// 错误消息
    Error {
        /// 错误代码
        code: u32,
        /// 错误描述
        message: String,
    },
}

impl WdicMessage {
    /// 创建广播消息
    /// 
    /// # 参数
    /// 
    /// * `sender` - 发送者信息
    /// 
    /// # 返回值
    /// 
    /// 广播消息实例
    pub fn broadcast(sender: RegistryEntry) -> Self {
        Self::Broadcast { sender }
    }

    /// 创建广播响应消息
    /// 
    /// # 参数
    /// 
    /// * `sender` - 响应者信息
    /// * `gateways` - 已知网关列表
    /// 
    /// # 返回值
    /// 
    /// 广播响应消息实例
    pub fn broadcast_response(sender: RegistryEntry, gateways: Vec<RegistryEntry>) -> Self {
        Self::BroadcastResponse { sender, gateways }
    }

    /// 创建心跳消息
    /// 
    /// # 参数
    /// 
    /// * `sender_id` - 发送者 ID
    /// 
    /// # 返回值
    /// 
    /// 心跳消息实例
    pub fn heartbeat(sender_id: Uuid) -> Self {
        Self::Heartbeat {
            sender_id,
            timestamp: chrono::Utc::now(),
        }
    }

    /// 创建心跳响应消息
    /// 
    /// # 参数
    /// 
    /// * `sender_id` - 响应者 ID
    /// 
    /// # 返回值
    /// 
    /// 心跳响应消息实例
    pub fn heartbeat_response(sender_id: Uuid) -> Self {
        Self::HeartbeatResponse {
            sender_id,
            timestamp: chrono::Utc::now(),
        }
    }

    /// 创建注册请求消息
    /// 
    /// # 参数
    /// 
    /// * `gateway` - 要注册的网关信息
    /// 
    /// # 返回值
    /// 
    /// 注册请求消息实例
    pub fn register_request(gateway: RegistryEntry) -> Self {
        Self::RegisterRequest { gateway }
    }

    /// 创建注册响应消息
    /// 
    /// # 参数
    /// 
    /// * `success` - 是否成功
    /// * `message` - 响应消息
    /// * `gateways` - 网关列表
    /// 
    /// # 返回值
    /// 
    /// 注册响应消息实例
    pub fn register_response(success: bool, message: String, gateways: Vec<RegistryEntry>) -> Self {
        Self::RegisterResponse {
            success,
            message,
            gateways,
        }
    }

    /// 创建查询网关列表消息
    /// 
    /// # 参数
    /// 
    /// * `requester_id` - 查询者 ID
    /// 
    /// # 返回值
    /// 
    /// 查询消息实例
    pub fn query_gateways(requester_id: Uuid) -> Self {
        Self::QueryGateways { requester_id }
    }

    /// 创建查询响应消息
    /// 
    /// # 参数
    /// 
    /// * `sender_id` - 响应者 ID
    /// * `gateways` - 网关列表
    /// 
    /// # 返回值
    /// 
    /// 查询响应消息实例
    pub fn query_response(sender_id: Uuid, gateways: Vec<RegistryEntry>) -> Self {
        Self::QueryResponse { sender_id, gateways }
    }

    /// 创建错误消息
    /// 
    /// # 参数
    /// 
    /// * `code` - 错误代码
    /// * `message` - 错误描述
    /// 
    /// # 返回值
    /// 
    /// 错误消息实例
    pub fn error(code: u32, message: String) -> Self {
        Self::Error { code, message }
    }

    /// 序列化消息为字节
    /// 
    /// # 返回值
    /// 
    /// 序列化结果，成功时返回字节向量
    pub fn to_bytes(&self) -> anyhow::Result<Vec<u8>> {
        serde_json::to_vec(self).map_err(|e| anyhow::anyhow!("序列化消息失败: {}", e))
    }

    /// 从字节反序列化消息
    /// 
    /// # 参数
    /// 
    /// * `bytes` - 字节数据
    /// 
    /// # 返回值
    /// 
    /// 反序列化结果，成功时返回消息实例
    pub fn from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        serde_json::from_slice(bytes).map_err(|e| anyhow::anyhow!("反序列化消息失败: {}", e))
    }

    /// 获取消息类型字符串
    /// 
    /// # 返回值
    /// 
    /// 消息类型的字符串表示
    pub fn message_type(&self) -> &'static str {
        match self {
            Self::Broadcast { .. } => "Broadcast",
            Self::BroadcastResponse { .. } => "BroadcastResponse",
            Self::Heartbeat { .. } => "Heartbeat",
            Self::HeartbeatResponse { .. } => "HeartbeatResponse",
            Self::RegisterRequest { .. } => "RegisterRequest",
            Self::RegisterResponse { .. } => "RegisterResponse",
            Self::UnregisterRequest { .. } => "UnregisterRequest",
            Self::UnregisterResponse { .. } => "UnregisterResponse",
            Self::QueryGateways { .. } => "QueryGateways",
            Self::QueryResponse { .. } => "QueryResponse",
            Self::Error { .. } => "Error",
        }
    }

    /// 获取发送者 ID（如果消息包含）
    /// 
    /// # 返回值
    /// 
    /// 发送者 ID，如果消息不包含发送者信息则返回 None
    pub fn sender_id(&self) -> Option<Uuid> {
        match self {
            Self::Broadcast { sender } => Some(sender.id),
            Self::BroadcastResponse { sender, .. } => Some(sender.id),
            Self::Heartbeat { sender_id, .. } => Some(*sender_id),
            Self::HeartbeatResponse { sender_id, .. } => Some(*sender_id),
            Self::RegisterRequest { gateway } => Some(gateway.id),
            Self::QueryGateways { requester_id } => Some(*requester_id),
            Self::QueryResponse { sender_id, .. } => Some(*sender_id),
            Self::UnregisterRequest { gateway_id } => Some(*gateway_id),
            _ => None,
        }
    }
}

/// WDIC 协议处理器
/// 
/// 负责处理 WDIC 协议消息的编码、解码和路由。
#[derive(Debug, Clone)]
pub struct WdicProtocol {
    /// 协议版本
    version: String,
}

impl WdicProtocol {
    /// 创建新的协议处理器
    /// 
    /// # 返回值
    /// 
    /// 协议处理器实例
    pub fn new() -> Self {
        Self {
            version: "1.0.0".to_string(),
        }
    }

    /// 获取协议版本
    /// 
    /// # 返回值
    /// 
    /// 协议版本字符串
    pub fn version(&self) -> &str {
        &self.version
    }

    /// 验证消息格式
    /// 
    /// # 参数
    /// 
    /// * `message` - 要验证的消息
    /// 
    /// # 返回值
    /// 
    /// 验证结果，成功时返回 ()
    pub fn validate_message(&self, message: &WdicMessage) -> anyhow::Result<()> {
        match message {
            WdicMessage::Broadcast { sender } => {
                if sender.name.is_empty() {
                    return Err(anyhow::anyhow!("广播消息发送者名称不能为空"));
                }
                if sender.address.port() == 0 {
                    return Err(anyhow::anyhow!("广播消息发送者端口无效"));
                }
            }
            WdicMessage::BroadcastResponse { sender, gateways } => {
                if sender.name.is_empty() {
                    return Err(anyhow::anyhow!("广播响应发送者名称不能为空"));
                }
                for gateway in gateways {
                    if gateway.name.is_empty() {
                        return Err(anyhow::anyhow!("网关列表中存在空名称"));
                    }
                }
            }
            WdicMessage::RegisterRequest { gateway } => {
                if gateway.name.is_empty() {
                    return Err(anyhow::anyhow!("注册请求网关名称不能为空"));
                }
                if gateway.address.port() == 0 {
                    return Err(anyhow::anyhow!("注册请求网关端口无效"));
                }
            }
            WdicMessage::Error { code, message } => {
                if *code == 0 {
                    return Err(anyhow::anyhow!("错误代码不能为0"));
                }
                if message.is_empty() {
                    return Err(anyhow::anyhow!("错误消息不能为空"));
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// 处理接收到的消息
    /// 
    /// # 参数
    /// 
    /// * `message` - 接收到的消息
    /// * `sender_addr` - 发送者地址
    /// 
    /// # 返回值
    /// 
    /// 处理结果，包含可能的响应消息
    pub fn handle_message(
        &self,
        message: &WdicMessage,
        sender_addr: SocketAddr,
    ) -> anyhow::Result<Option<WdicMessage>> {
        // 验证消息格式
        self.validate_message(message)?;

        log::info!(
            "收到来自 {} 的 {} 消息",
            sender_addr,
            message.message_type()
        );

        // 这里返回 None，实际的消息处理将在网关层实现
        // 这个方法主要用于消息验证和日志记录
        Ok(None)
    }
}

impl Default for WdicProtocol {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::RegistryEntry;
    use std::net::{IpAddr, Ipv4Addr};

    fn create_test_entry(name: &str, port: u16) -> RegistryEntry {
        let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)), port);
        RegistryEntry::new(name.to_string(), address)
    }

    #[test]
    fn test_wdic_message_broadcast() {
        let entry = create_test_entry("测试网关", 55555);
        let entry_id = entry.id;
        let message = WdicMessage::broadcast(entry);

        match &message {
            WdicMessage::Broadcast { sender } => {
                assert_eq!(sender.name, "测试网关");
                assert_eq!(sender.address.port(), 55555);
            }
            _ => panic!("消息类型不正确"),
        }

        assert_eq!(message.message_type(), "Broadcast");
        assert_eq!(message.sender_id(), Some(entry_id));
    }

    #[test]
    fn test_wdic_message_broadcast_response() {
        let sender = create_test_entry("发送者", 55555);
        let sender_id = sender.id;
        let gateway1 = create_test_entry("网关1", 55556);
        let gateway2 = create_test_entry("网关2", 55557);
        let gateways = vec![gateway1, gateway2];

        let message = WdicMessage::broadcast_response(sender, gateways);

        match &message {
            WdicMessage::BroadcastResponse { sender: s, gateways: g } => {
                assert_eq!(s.name, "发送者");
                assert_eq!(g.len(), 2);
                assert_eq!(g[0].name, "网关1");
                assert_eq!(g[1].name, "网关2");
            }
            _ => panic!("消息类型不正确"),
        }

        assert_eq!(message.message_type(), "BroadcastResponse");
        assert_eq!(message.sender_id(), Some(sender_id));
    }

    #[test]
    fn test_wdic_message_heartbeat() {
        let id = uuid::Uuid::new_v4();
        let message = WdicMessage::heartbeat(id);

        match message {
            WdicMessage::Heartbeat { sender_id, timestamp } => {
                assert_eq!(sender_id, id);
                assert!(timestamp <= chrono::Utc::now());
            }
            _ => panic!("消息类型不正确"),
        }

        assert_eq!(message.message_type(), "Heartbeat");
        assert_eq!(message.sender_id(), Some(id));
    }

    #[test]
    fn test_wdic_message_serialization() {
        let entry = create_test_entry("测试网关", 55555);
        let message = WdicMessage::broadcast(entry);

        let bytes = message.to_bytes().expect("序列化失败");
        assert!(!bytes.is_empty());

        let deserialized = WdicMessage::from_bytes(&bytes).expect("反序列化失败");
        assert_eq!(message, deserialized);
    }

    #[test]
    fn test_wdic_message_error() {
        let message = WdicMessage::error(404, "网关未找到".to_string());

        match &message {
            WdicMessage::Error { code, message: msg } => {
                assert_eq!(*code, 404);
                assert_eq!(msg, "网关未找到");
            }
            _ => panic!("消息类型不正确"),
        }

        assert_eq!(message.message_type(), "Error");
        assert_eq!(message.sender_id(), None);
    }

    #[test]
    fn test_wdic_protocol_creation() {
        let protocol = WdicProtocol::new();
        assert_eq!(protocol.version(), "1.0.0");
    }

    #[test]
    fn test_wdic_protocol_validate_message() {
        let protocol = WdicProtocol::new();

        // 有效的广播消息
        let entry = create_test_entry("测试网关", 55555);
        let valid_message = WdicMessage::broadcast(entry);
        assert!(protocol.validate_message(&valid_message).is_ok());

        // 无效的广播消息（空名称）
        let invalid_entry = create_test_entry("", 55555);
        let invalid_message = WdicMessage::broadcast(invalid_entry);
        assert!(protocol.validate_message(&invalid_message).is_err());

        // 无效的错误消息（错误代码为0）
        let invalid_error = WdicMessage::error(0, "测试错误".to_string());
        assert!(protocol.validate_message(&invalid_error).is_err());
    }

    #[test]
    fn test_wdic_protocol_handle_message() {
        let protocol = WdicProtocol::new();
        let entry = create_test_entry("测试网关", 55555);
        let message = WdicMessage::broadcast(entry);
        let sender_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 2)), 55556);

        let result = protocol.handle_message(&message, sender_addr);
        assert!(result.is_ok());
        // 当前实现返回 None，实际处理在网关层
        assert!(result.unwrap().is_none());
    }
}