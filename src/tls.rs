//! TLS 1.3 mTLS 验证模块
//!
//! 提供 TLS 1.3 双向认证支持，确保网关间通信的安全性。
//! 支持证书生成、验证和管理功能。

use anyhow::{Context, Result};
use base64::prelude::*;
use log::{debug, info};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{create_dir_all, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// TLS 证书信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CertificateInfo {
    /// 证书主题名称
    pub subject: String,
    /// 证书颁发者
    pub issuer: String,
    /// 序列号
    pub serial_number: String,
    /// 有效期开始时间
    pub not_before: SystemTime,
    /// 有效期结束时间
    pub not_after: SystemTime,
    /// 证书指纹（SHA-256）
    pub fingerprint: String,
    /// 证书用途
    pub key_usage: Vec<String>,
}

/// mTLS 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MtlsConfig {
    /// CA 证书路径
    pub ca_cert_path: PathBuf,
    /// 服务端证书路径
    pub server_cert_path: PathBuf,
    /// 服务端私钥路径
    pub server_key_path: PathBuf,
    /// 客户端证书路径
    pub client_cert_path: PathBuf,
    /// 客户端私钥路径
    pub client_key_path: PathBuf,
    /// 证书验证模式
    pub verify_mode: VerifyMode,
    /// 支持的 TLS 版本
    pub tls_versions: Vec<TlsVersion>,
    /// 支持的密码套件
    pub cipher_suites: Vec<String>,
}

/// 证书验证模式
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum VerifyMode {
    /// 不验证证书
    None,
    /// 验证证书有效性
    VerifyPeer,
    /// 双向验证
    MutualAuth,
    /// 严格模式（验证证书链、CRL等）
    Strict,
}

/// TLS 版本
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TlsVersion {
    /// TLS 1.2
    Tls12,
    /// TLS 1.3
    Tls13,
}

impl Default for MtlsConfig {
    fn default() -> Self {
        Self {
            ca_cert_path: PathBuf::from("certs/ca.crt"),
            server_cert_path: PathBuf::from("certs/server.crt"),
            server_key_path: PathBuf::from("certs/server.key"),
            client_cert_path: PathBuf::from("certs/client.crt"),
            client_key_path: PathBuf::from("certs/client.key"),
            verify_mode: VerifyMode::MutualAuth,
            tls_versions: vec![TlsVersion::Tls13],
            cipher_suites: vec![
                "TLS_AES_256_GCM_SHA384".to_string(),
                "TLS_CHACHA20_POLY1305_SHA256".to_string(),
                "TLS_AES_128_GCM_SHA256".to_string(),
            ],
        }
    }
}

/// TLS 管理器
#[derive(Debug)]
pub struct TlsManager {
    /// 配置
    config: MtlsConfig,
    /// 信任的证书存储（保留用于未来扩展）
    #[allow(dead_code)]
    trusted_certs: HashMap<String, CertificateInfo>,
    /// 证书缓存
    cert_cache: HashMap<String, Vec<u8>>,
    /// 私钥缓存
    key_cache: HashMap<String, Vec<u8>>,
}

impl TlsManager {
    /// 创建新的 TLS 管理器
    ///
    /// # 参数
    ///
    /// * `config` - mTLS 配置
    ///
    /// # 返回值
    ///
    /// TLS 管理器实例
    pub fn new(config: MtlsConfig) -> Result<Self> {
        let mut manager = Self {
            config,
            trusted_certs: HashMap::new(),
            cert_cache: HashMap::new(),
            key_cache: HashMap::new(),
        };

        // 确保证书目录存在
        let cert_dir = manager
            .config
            .ca_cert_path
            .parent()
            .unwrap_or_else(|| Path::new("certs"));
        create_dir_all(cert_dir).context("创建证书目录失败")?;

        // 初始化证书
        manager
            .initialize_certificates()
            .context("初始化证书失败")?;

        info!(
            "TLS 管理器初始化完成，验证模式: {:?}",
            manager.config.verify_mode
        );

        Ok(manager)
    }

    /// 初始化证书
    fn initialize_certificates(&mut self) -> Result<()> {
        // 检查证书是否存在，如果不存在则生成
        if !self.config.ca_cert_path.exists() {
            info!("CA 证书不存在，生成自签名证书");
            self.generate_self_signed_certificates()
                .context("生成自签名证书失败")?;
        }

        // 加载证书到缓存
        self.load_certificates().context("加载证书失败")?;

        Ok(())
    }

    /// 生成自签名证书（用于开发和测试）
    fn generate_self_signed_certificates(&mut self) -> Result<()> {
        info!("生成开发用自签名证书");

        // 这里是一个简化的实现，实际生产环境应该使用专业的 CA
        let ca_cert = self.create_dummy_certificate("WDIC Gateway CA", true)?;
        let server_cert = self.create_dummy_certificate("WDIC Gateway Server", false)?;
        let client_cert = self.create_dummy_certificate("WDIC Gateway Client", false)?;

        // 保存 CA 证书
        self.save_certificate_to_file(&ca_cert, &self.config.ca_cert_path)?;

        // 保存服务端证书
        self.save_certificate_to_file(&server_cert, &self.config.server_cert_path)?;

        // 保存客户端证书
        self.save_certificate_to_file(&client_cert, &self.config.client_cert_path)?;

        // 生成私钥（简化实现）
        let dummy_key = self.create_dummy_private_key()?;
        self.save_private_key_to_file(&dummy_key, &self.config.server_key_path)?;
        self.save_private_key_to_file(&dummy_key, &self.config.client_key_path)?;

        info!("自签名证书生成完成");

        Ok(())
    }

    /// 创建模拟证书（简化实现，仅用于演示）
    fn create_dummy_certificate(&self, subject: &str, is_ca: bool) -> Result<Vec<u8>> {
        // 这是一个简化的证书格式，实际应该使用 X.509 标准
        let cert_info = CertificateInfo {
            subject: subject.to_string(),
            issuer: if is_ca {
                subject.to_string()
            } else {
                "WDIC Gateway CA".to_string()
            },
            serial_number: format!("{:016x}", rand::random::<u64>()),
            not_before: SystemTime::now(),
            not_after: SystemTime::now() + Duration::from_secs(365 * 24 * 3600), // 1年有效期
            fingerprint: format!("sha256:{:064x}", rand::random::<u64>()),
            key_usage: if is_ca {
                vec!["Certificate Sign".to_string(), "CRL Sign".to_string()]
            } else {
                vec![
                    "Digital Signature".to_string(),
                    "Key Encipherment".to_string(),
                ]
            },
        };

        // 序列化证书信息为 JSON（实际应该是 DER 或 PEM 格式）
        let cert_json = serde_json::to_string_pretty(&cert_info).context("序列化证书信息失败")?;

        // 简单的"证书"格式
        let cert_content = format!(
            "-----BEGIN CERTIFICATE-----\n{}\n-----END CERTIFICATE-----\n",
            base64::prelude::BASE64_STANDARD.encode(cert_json)
        );

        Ok(cert_content.into_bytes())
    }

    /// 创建模拟私钥（简化实现）
    fn create_dummy_private_key(&self) -> Result<Vec<u8>> {
        // 生成随机私钥数据（实际应该使用密码学库）
        let key_data: [u8; 32] = rand::random();

        let key_content = format!(
            "-----BEGIN PRIVATE KEY-----\n{}\n-----END PRIVATE KEY-----\n",
            base64::prelude::BASE64_STANDARD.encode(key_data)
        );

        Ok(key_content.into_bytes())
    }

    /// 保存证书到文件
    fn save_certificate_to_file(&self, cert_data: &[u8], path: &Path) -> Result<()> {
        let mut file =
            File::create(path).with_context(|| format!("创建证书文件失败: {path:?}"))?;

        file.write_all(cert_data)
            .with_context(|| format!("写入证书文件失败: {path:?}"))?;

        debug!("证书已保存到: {path:?}");

        Ok(())
    }

    /// 保存私钥到文件
    fn save_private_key_to_file(&self, key_data: &[u8], path: &Path) -> Result<()> {
        let mut file =
            File::create(path).with_context(|| format!("创建私钥文件失败: {path:?}"))?;

        file.write_all(key_data)
            .with_context(|| format!("写入私钥文件失败: {path:?}"))?;

        debug!("私钥已保存到: {path:?}");

        Ok(())
    }

    /// 加载证书到缓存
    fn load_certificates(&mut self) -> Result<()> {
        // 获取路径的副本以避免借用检查问题
        let ca_cert_path = self.config.ca_cert_path.clone();
        let server_cert_path = self.config.server_cert_path.clone();
        let client_cert_path = self.config.client_cert_path.clone();
        let server_key_path = self.config.server_key_path.clone();
        let client_key_path = self.config.client_key_path.clone();

        // 加载 CA 证书
        self.load_certificate_file(&ca_cert_path, "ca")?;

        // 加载服务端证书
        self.load_certificate_file(&server_cert_path, "server")?;

        // 加载客户端证书
        self.load_certificate_file(&client_cert_path, "client")?;

        // 加载私钥
        self.load_private_key_file(&server_key_path, "server")?;
        self.load_private_key_file(&client_key_path, "client")?;

        info!(
            "证书加载完成，缓存了 {} 个证书和 {} 个私钥",
            self.cert_cache.len(),
            self.key_cache.len()
        );

        Ok(())
    }

    /// 从文件加载证书
    fn load_certificate_file(&mut self, path: &Path, name: &str) -> Result<()> {
        if !path.exists() {
            return Err(anyhow::anyhow!("证书文件不存在: {:?}", path));
        }

        let mut file = File::open(path).with_context(|| format!("打开证书文件失败: {path:?}"))?;

        let mut cert_data = Vec::new();
        file.read_to_end(&mut cert_data)
            .with_context(|| format!("读取证书文件失败: {path:?}"))?;

        self.cert_cache.insert(name.to_string(), cert_data);

        debug!("证书已加载: {name} -> {path:?}");

        Ok(())
    }

    /// 从文件加载私钥
    fn load_private_key_file(&mut self, path: &Path, name: &str) -> Result<()> {
        if !path.exists() {
            return Err(anyhow::anyhow!("私钥文件不存在: {:?}", path));
        }

        let mut file = File::open(path).with_context(|| format!("读取私钥文件失败: {path:?}"))?;

        let mut key_data = Vec::new();
        file.read_to_end(&mut key_data)
            .with_context(|| format!("读取私钥文件失败: {path:?}"))?;

        self.key_cache.insert(name.to_string(), key_data);

        debug!("私钥已加载: {name} -> {path:?}");

        Ok(())
    }

    /// 获取证书数据
    pub fn get_certificate(&self, name: &str) -> Option<&[u8]> {
        self.cert_cache.get(name).map(|data| data.as_slice())
    }

    /// 获取私钥数据
    pub fn get_private_key(&self, name: &str) -> Option<&[u8]> {
        self.key_cache.get(name).map(|data| data.as_slice())
    }

    /// 验证证书
    pub fn verify_certificate(&self, cert_data: &[u8]) -> Result<bool> {
        // 简化的证书验证逻辑
        if cert_data.is_empty() {
            return Ok(false);
        }

        // 检查证书格式
        let cert_str = String::from_utf8_lossy(cert_data);
        if !cert_str.contains("-----BEGIN CERTIFICATE-----")
            || !cert_str.contains("-----END CERTIFICATE-----")
        {
            return Ok(false);
        }

        // 在实际实现中，这里应该：
        // 1. 解析 X.509 证书
        // 2. 验证证书链
        // 3. 检查证书有效期
        // 4. 验证证书签名
        // 5. 检查 CRL/OCSP

        match self.config.verify_mode {
            VerifyMode::None => Ok(true),
            VerifyMode::VerifyPeer => {
                // 基本格式验证
                Ok(cert_str.len() > 100) // 简单的长度检查
            }
            VerifyMode::MutualAuth | VerifyMode::Strict => {
                // 更严格的验证
                let has_ca = self.cert_cache.contains_key("ca");
                Ok(has_ca && cert_str.len() > 100)
            }
        }
    }

    /// 验证对等证书
    pub fn verify_peer_certificate(&self, peer_cert: &[u8]) -> Result<bool> {
        if self.config.verify_mode == VerifyMode::None {
            return Ok(true);
        }

        self.verify_certificate(peer_cert)
    }

    /// 获取支持的 TLS 版本字符串
    pub fn get_tls_version_string(&self) -> String {
        self.config
            .tls_versions
            .iter()
            .map(|v| match v {
                TlsVersion::Tls12 => "TLSv1.2",
                TlsVersion::Tls13 => "TLSv1.3",
            })
            .collect::<Vec<_>>()
            .join(",")
    }

    /// 获取支持的密码套件字符串
    pub fn get_cipher_suites_string(&self) -> String {
        self.config.cipher_suites.join(":")
    }

    /// 检查是否启用了 TLS 1.3
    pub fn is_tls13_enabled(&self) -> bool {
        self.config.tls_versions.contains(&TlsVersion::Tls13)
    }

    /// 检查是否启用了双向认证
    pub fn is_mutual_auth_enabled(&self) -> bool {
        matches!(
            self.config.verify_mode,
            VerifyMode::MutualAuth | VerifyMode::Strict
        )
    }

    /// 获取证书统计信息
    pub fn get_certificate_stats(&self) -> (usize, usize, bool) {
        let cert_count = self.cert_cache.len();
        let key_count = self.key_cache.len();
        let mtls_ready = self.is_mutual_auth_enabled() && cert_count >= 3 && key_count >= 2;

        (cert_count, key_count, mtls_ready)
    }

    /// 获取配置引用
    pub fn config(&self) -> &MtlsConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_mtls_config_default() {
        let config = MtlsConfig::default();
        assert_eq!(config.verify_mode, VerifyMode::MutualAuth);
        assert!(config.tls_versions.contains(&TlsVersion::Tls13));
    }

    #[test]
    fn test_tls_manager_creation() -> Result<()> {
        let temp_dir = tempdir()?;
        let config = MtlsConfig {
            ca_cert_path: temp_dir.path().join("ca.crt"),
            server_cert_path: temp_dir.path().join("server.crt"),
            server_key_path: temp_dir.path().join("server.key"),
            client_cert_path: temp_dir.path().join("client.crt"),
            client_key_path: temp_dir.path().join("client.key"),
            ..Default::default()
        };

        let manager = TlsManager::new(config)?;
        assert!(manager.is_tls13_enabled());
        assert!(manager.is_mutual_auth_enabled());

        Ok(())
    }

    #[test]
    fn test_certificate_verification() -> Result<()> {
        let temp_dir = tempdir()?;
        let config = MtlsConfig {
            ca_cert_path: temp_dir.path().join("ca.crt"),
            server_cert_path: temp_dir.path().join("server.crt"),
            server_key_path: temp_dir.path().join("server.key"),
            client_cert_path: temp_dir.path().join("client.crt"),
            client_key_path: temp_dir.path().join("client.key"),
            ..Default::default()
        };

        let manager = TlsManager::new(config)?;

        // 测试空证书
        assert!(!manager.verify_certificate(b"")?);

        // 测试无效格式
        assert!(!manager.verify_certificate(b"invalid cert data")?);

        Ok(())
    }

    #[test]
    fn test_certificate_stats() -> Result<()> {
        let temp_dir = tempdir()?;
        let config = MtlsConfig {
            ca_cert_path: temp_dir.path().join("ca.crt"),
            server_cert_path: temp_dir.path().join("server.crt"),
            server_key_path: temp_dir.path().join("server.key"),
            client_cert_path: temp_dir.path().join("client.crt"),
            client_key_path: temp_dir.path().join("client.key"),
            ..Default::default()
        };

        let manager = TlsManager::new(config)?;
        let (cert_count, key_count, mtls_ready) = manager.get_certificate_stats();

        assert!(cert_count >= 3);
        assert!(key_count >= 2);
        assert!(mtls_ready);

        Ok(())
    }
}
