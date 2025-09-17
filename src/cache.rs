//! 网关缓存系统模块
//!
//! 实现网关缓存系统，支持文件名称哈希、zstd压缩、元数据管理和缓存失效机制。
//! 缓存文件格式：\[固定大小元数据\]\[压缩后的文件数据\]\[20字节随机后缀\]

use anyhow::{Context, Result};
use chrono::{serde::ts_seconds, DateTime, Utc};
use log::{debug, info, warn};
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

/// 缓存元数据结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheMetadata {
    /// 原始文件名称
    pub original_name: String,
    /// 文件大小（压缩前）
    pub original_size: u64,
    /// 压缩后大小
    pub compressed_size: u64,
    /// 文件哈希值
    pub file_hash: String,
    /// 创建时间
    #[serde(with = "ts_seconds")]
    pub created_at: DateTime<Utc>,
    /// 失效时间
    #[serde(with = "ts_seconds")]
    pub expires_at: DateTime<Utc>,
    /// MIME 类型
    pub mime_type: String,
    /// 压缩比率
    pub compression_ratio: f64,
    /// 版本号
    pub version: u32,
}

impl CacheMetadata {
    /// 固定的元数据字节大小（512字节）
    pub const METADATA_SIZE: usize = 512;

    /// 创建新的缓存元数据
    pub fn new(
        original_name: String,
        original_size: u64,
        compressed_size: u64,
        file_hash: String,
        ttl_seconds: u64,
    ) -> Self {
        let now = Utc::now();
        let expires_at = now + chrono::Duration::seconds(ttl_seconds as i64);

        // 简单的MIME类型检测
        let mime_type = Self::detect_mime_type(&original_name);
        let compression_ratio = if original_size > 0 {
            compressed_size as f64 / original_size as f64
        } else {
            1.0
        };

        Self {
            original_name,
            original_size,
            compressed_size,
            file_hash,
            created_at: now,
            expires_at,
            mime_type,
            compression_ratio,
            version: 1,
        }
    }

    /// 检测文件的MIME类型
    fn detect_mime_type(filename: &str) -> String {
        let extension = Path::new(filename)
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
            .to_lowercase();

        match extension.as_str() {
            "txt" => "text/plain",
            "html" | "htm" => "text/html",
            "css" => "text/css",
            "js" => "application/javascript",
            "json" => "application/json",
            "xml" => "application/xml",
            "pdf" => "application/pdf",
            "zip" => "application/zip",
            "tar" => "application/x-tar",
            "gz" => "application/gzip",
            "jpg" | "jpeg" => "image/jpeg",
            "png" => "image/png",
            "gif" => "image/gif",
            "svg" => "image/svg+xml",
            "mp3" => "audio/mpeg",
            "mp4" => "video/mp4",
            "avi" => "video/x-msvideo",
            "mov" => "video/quicktime",
            _ => "application/octet-stream",
        }
        .to_string()
    }

    /// 序列化为固定大小的字节数组
    pub fn to_fixed_bytes(&self) -> Result<[u8; Self::METADATA_SIZE]> {
        let json_data = serde_json::to_string(self).context("序列化元数据失败")?;

        let mut bytes = [0u8; Self::METADATA_SIZE];
        let json_bytes = json_data.as_bytes();

        if json_bytes.len() > Self::METADATA_SIZE - 3 {
            return Err(anyhow::anyhow!("元数据过大，无法序列化到固定大小"));
        }

        // 复制JSON数据
        bytes[..json_bytes.len()].copy_from_slice(json_bytes);

        // 剩余空间用随机字符填充
        let mut rng = rand::rng();
        for byte in bytes
            .iter_mut()
            .take(Self::METADATA_SIZE - 2)
            .skip(json_bytes.len())
        {
            *byte = rng.random::<u8>() % 94 + 33; // 可打印ASCII字符
        }

        // 最后两个字节存储JSON数据的实际长度（小端序）
        let json_len_bytes = (json_bytes.len() as u16).to_le_bytes();
        bytes[Self::METADATA_SIZE - 2] = json_len_bytes[0];
        bytes[Self::METADATA_SIZE - 1] = json_len_bytes[1];

        Ok(bytes)
    }

    /// 从固定大小的字节数组反序列化
    pub fn from_fixed_bytes(bytes: &[u8; Self::METADATA_SIZE]) -> Result<Self> {
        // 从最后两个字节读取JSON数据的实际长度
        let json_len = u16::from_le_bytes([
            bytes[Self::METADATA_SIZE - 2],
            bytes[Self::METADATA_SIZE - 1],
        ]) as usize;

        if json_len >= Self::METADATA_SIZE - 2 {
            return Err(anyhow::anyhow!("无效的元数据长度标记: {}", json_len));
        }

        let json_data = std::str::from_utf8(&bytes[..json_len]).context("解析元数据UTF-8失败")?;

        serde_json::from_str(json_data).context("反序列化元数据失败")
    }

    /// 检查是否已过期
    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }

    /// 获取剩余生存时间（秒）
    pub fn remaining_ttl(&self) -> i64 {
        (self.expires_at - Utc::now()).num_seconds().max(0)
    }
}

/// 缓存条目信息
#[derive(Debug, Clone)]
pub struct CacheEntry {
    /// 元数据
    pub metadata: CacheMetadata,
    /// 缓存文件路径
    pub cache_file_path: PathBuf,
    /// 访问计数
    pub access_count: u64,
    /// 最后访问时间
    pub last_accessed: DateTime<Utc>,
}

impl CacheEntry {
    /// 创建新的缓存条目
    pub fn new(metadata: CacheMetadata, cache_file_path: PathBuf) -> Self {
        Self {
            metadata,
            cache_file_path,
            access_count: 0,
            last_accessed: Utc::now(),
        }
    }

    /// 记录访问
    pub fn record_access(&mut self) {
        self.access_count += 1;
        self.last_accessed = Utc::now();
    }
}

/// 网关缓存系统
#[derive(Debug, Default)]
pub struct GatewayCache {
    /// 缓存目录
    cache_dir: PathBuf,
    /// 缓存条目索引（文件哈希 -> 缓存条目）
    cache_index: HashMap<String, CacheEntry>,
    /// 名称哈希索引（名称 -> 文件哈希）
    name_hash_index: HashMap<String, String>,
    /// 默认TTL（秒）
    default_ttl: u64,
    /// 最大缓存大小（字节）
    max_cache_size: u64,
    /// 当前缓存大小（字节）
    current_cache_size: u64,
}

impl GatewayCache {
    /// 创建新的网关缓存系统
    ///
    /// # 参数
    ///
    /// * `cache_dir` - 缓存目录路径
    /// * `default_ttl` - 默认生存时间（秒）
    /// * `max_cache_size` - 最大缓存大小（字节）
    ///
    /// # 返回值
    ///
    /// 缓存系统实例
    pub fn new<P: AsRef<Path>>(
        cache_dir: P,
        default_ttl: u64,
        max_cache_size: u64,
    ) -> Result<Self> {
        let cache_dir = cache_dir.as_ref().to_path_buf();

        // 创建缓存目录
        std::fs::create_dir_all(&cache_dir).context("创建缓存目录失败")?;

        let mut cache = Self {
            cache_dir,
            cache_index: HashMap::new(),
            name_hash_index: HashMap::new(),
            default_ttl,
            max_cache_size,
            current_cache_size: 0,
        };

        // 加载现有缓存
        cache.load_existing_cache().context("加载现有缓存失败")?;

        info!(
            "网关缓存系统初始化完成，缓存目录: {:?}, 当前大小: {} MB",
            cache.cache_dir,
            cache.current_cache_size / (1024 * 1024)
        );

        Ok(cache)
    }

    /// 计算文件名称的哈希值
    pub fn calculate_name_hash(name: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(name.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// 计算文件内容的哈希值
    pub fn calculate_content_hash(data: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data);
        format!("{:x}", hasher.finalize())
    }

    /// 生成20字节随机后缀
    fn generate_random_suffix() -> [u8; 20] {
        let mut rng = rand::rng();
        let mut suffix = [0u8; 20];
        for byte in &mut suffix {
            *byte = rng.random::<u8>();
        }
        suffix
    }

    /// 缓存文件
    ///
    /// # 参数
    ///
    /// * `name` - 文件名称
    /// * `data` - 文件数据
    /// * `ttl` - 生存时间（可选，使用默认值如果为None）
    ///
    /// # 返回值
    ///
    /// 文件哈希值
    pub fn cache_file(&mut self, name: &str, data: &[u8], ttl: Option<u64>) -> Result<String> {
        let name_hash = Self::calculate_name_hash(name);
        let content_hash = Self::calculate_content_hash(data);
        let ttl = ttl.unwrap_or(self.default_ttl);

        debug!(
            "缓存文件: {name}, 名称哈希: {name_hash}, 内容哈希: {content_hash}"
        );

        // 检查是否需要清理空间
        self.ensure_cache_space(data.len() as u64 * 2) // 预留压缩后的空间
            .context("清理缓存空间失败")?;

        // 压缩数据
        let compressed_data = zstd::encode_all(data, 3).context("压缩文件数据失败")?;

        debug!(
            "压缩完成: {} 字节 -> {} 字节 (压缩率: {:.2}%)",
            data.len(),
            compressed_data.len(),
            (compressed_data.len() as f64 / data.len() as f64) * 100.0
        );

        // 创建元数据
        let metadata = CacheMetadata::new(
            name.to_string(),
            data.len() as u64,
            compressed_data.len() as u64,
            content_hash.clone(),
            ttl,
        );

        // 序列化元数据
        let metadata_bytes = metadata.to_fixed_bytes().context("序列化缓存元数据失败")?;

        // 生成随机后缀
        let random_suffix = Self::generate_random_suffix();

        // 创建缓存文件
        let cache_filename = format!("{name_hash}.cach");
        let cache_file_path = self.cache_dir.join(&cache_filename);

        let mut cache_file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&cache_file_path)
            .context("创建缓存文件失败")?;

        // 写入数据：[元数据][压缩数据][随机后缀]
        cache_file
            .write_all(&metadata_bytes)
            .context("写入元数据失败")?;
        cache_file
            .write_all(&compressed_data)
            .context("写入压缩数据失败")?;
        cache_file
            .write_all(&random_suffix)
            .context("写入随机后缀失败")?;

        cache_file.flush().context("刷新缓存文件失败")?;

        let cache_file_size = metadata_bytes.len() + compressed_data.len() + random_suffix.len();

        // 更新索引
        let cache_entry = CacheEntry::new(metadata, cache_file_path);
        self.cache_index.insert(content_hash.clone(), cache_entry);
        self.name_hash_index
            .insert(name_hash.clone(), content_hash.clone());
        self.current_cache_size += cache_file_size as u64;

        info!(
            "文件缓存成功: {name} -> {name_hash}.cach (大小: {cache_file_size} 字节)"
        );

        Ok(content_hash)
    }

    /// 获取缓存文件
    ///
    /// # 参数
    ///
    /// * `file_hash` - 文件哈希值
    ///
    /// # 返回值
    ///
    /// 解压缩后的文件数据和元数据
    pub fn get_cached_file(&mut self, file_hash: &str) -> Result<Option<(Vec<u8>, CacheMetadata)>> {
        if let Some(cache_entry) = self.cache_index.get_mut(file_hash) {
            // 检查是否过期
            if cache_entry.metadata.is_expired() {
                debug!("缓存文件已过期: {file_hash}");
                self.remove_cache_entry(file_hash)?;
                return Ok(None);
            }

            // 记录访问
            cache_entry.record_access();

            // 读取缓存文件
            let mut cache_file =
                File::open(&cache_entry.cache_file_path).context("打开缓存文件失败")?;

            // 读取元数据
            let mut metadata_bytes = [0u8; CacheMetadata::METADATA_SIZE];
            cache_file
                .read_exact(&mut metadata_bytes)
                .context("读取元数据失败")?;

            let metadata =
                CacheMetadata::from_fixed_bytes(&metadata_bytes).context("解析元数据失败")?;

            // 读取压缩数据
            let mut compressed_data = vec![0u8; metadata.compressed_size as usize];
            cache_file
                .read_exact(&mut compressed_data)
                .context("读取压缩数据失败")?;

            // 解压缩数据
            let decompressed_data =
                zstd::decode_all(&compressed_data[..]).context("解压缩文件数据失败")?;

            debug!(
                "缓存命中: {file_hash} (访问次数: {access_count})",
                access_count = cache_entry.access_count
            );

            Ok(Some((decompressed_data, metadata)))
        } else {
            debug!("缓存未命中: {file_hash}");
            Ok(None)
        }
    }

    /// 通过名称哈希获取缓存文件
    pub fn get_cached_file_by_name(
        &mut self,
        name: &str,
    ) -> Result<Option<(Vec<u8>, CacheMetadata)>> {
        let name_hash = Self::calculate_name_hash(name);
        if let Some(file_hash) = self.name_hash_index.get(&name_hash).cloned() {
            self.get_cached_file(&file_hash)
        } else {
            Ok(None)
        }
    }

    /// 获取所有缓存文件的名称哈希列表
    pub fn get_name_hash_list(&self) -> Vec<String> {
        self.name_hash_index.keys().cloned().collect()
    }

    /// 获取缓存统计信息
    ///
    /// # 返回值
    ///
    /// 缓存统计信息
    pub async fn get_stats(&self) -> crate::tauri_api::CacheStats {
        let total_entries = self.cache_index.len();
        
        // 这里应该从实际的统计数据获取命中率等信息
        // 目前使用模拟数据
        let hit_count = 100u64;
        let miss_count = 20u64;
        let hit_rate = if hit_count + miss_count > 0 {
            hit_count as f64 / (hit_count + miss_count) as f64
        } else {
            0.0
        };

        crate::tauri_api::CacheStats {
            item_count: total_entries,
            hit_count,
            miss_count,
            hit_rate,
            memory_usage: self.current_cache_size as usize,
            max_capacity: self.max_cache_size as usize,
        }
    }

    /// 健康检查
    ///
    /// # 返回值
    ///
    /// 健康状态
    pub async fn health_check(&self) -> bool {
        // 检查缓存目录是否存在和可访问
        self.cache_dir.exists() && self.cache_dir.is_dir()
    }

    /// 获取缓存统计信息
    pub fn get_cache_stats(&self) -> (usize, u64, u64) {
        (
            self.cache_index.len(),
            self.current_cache_size,
            self.max_cache_size,
        )
    }

    /// 清理过期缓存
    pub fn cleanup_expired(&mut self) -> Result<usize> {
        let mut expired_hashes = Vec::new();

        for (file_hash, entry) in &self.cache_index {
            if entry.metadata.is_expired() {
                expired_hashes.push(file_hash.clone());
            }
        }

        let count = expired_hashes.len();
        for hash in expired_hashes {
            self.remove_cache_entry(&hash)?;
        }

        if count > 0 {
            info!("清理了 {count} 个过期缓存条目");
        }

        Ok(count)
    }

    /// 确保有足够的缓存空间
    fn ensure_cache_space(&mut self, required_space: u64) -> Result<()> {
        if self.current_cache_size + required_space <= self.max_cache_size {
            return Ok(());
        }

        info!(
            "缓存空间不足，开始清理：当前 {} MB，需要 {} MB",
            self.current_cache_size / (1024 * 1024),
            required_space / (1024 * 1024)
        );

        // 首先清理过期条目
        self.cleanup_expired()?;

        if self.current_cache_size + required_space <= self.max_cache_size {
            return Ok(());
        }

        // 如果仍然空间不足，按LRU策略清理
        let mut entries: Vec<_> = self.cache_index.iter().collect();
        entries.sort_by_key(|(_, entry)| entry.last_accessed);

        let mut freed_space = 0u64;
        let mut to_remove = Vec::new();

        for (file_hash, entry) in entries {
            let file_size = entry
                .cache_file_path
                .metadata()
                .map(|m| m.len())
                .unwrap_or(0);

            freed_space += file_size;
            to_remove.push(file_hash.clone());

            if self.current_cache_size - freed_space + required_space <= self.max_cache_size {
                break;
            }
        }

        for hash in to_remove {
            self.remove_cache_entry(&hash)?;
        }

        info!(
            "LRU清理完成，释放了 {} MB 空间",
            freed_space / (1024 * 1024)
        );

        Ok(())
    }

    /// 移除缓存条目
    fn remove_cache_entry(&mut self, file_hash: &str) -> Result<()> {
        if let Some(entry) = self.cache_index.remove(file_hash) {
            // 删除文件
            if let Err(e) = std::fs::remove_file(&entry.cache_file_path) {
                warn!("删除缓存文件失败: {:?}, 错误: {}", entry.cache_file_path, e);
            } else {
                let file_size = entry
                    .cache_file_path
                    .metadata()
                    .map(|m| m.len())
                    .unwrap_or(0);
                self.current_cache_size = self.current_cache_size.saturating_sub(file_size);
            }

            // 从名称哈希索引中移除
            let name_hash = Self::calculate_name_hash(&entry.metadata.original_name);
            self.name_hash_index.remove(&name_hash);

            debug!("移除缓存条目: {file_hash}");
        }

        Ok(())
    }

    /// 加载现有缓存
    fn load_existing_cache(&mut self) -> Result<()> {
        let entries = std::fs::read_dir(&self.cache_dir).context("读取缓存目录失败")?;

        let mut loaded_count = 0;

        for entry in entries {
            let entry = entry.context("读取目录条目失败")?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("cach") {
                if let Err(e) = self.load_cache_file(&path) {
                    warn!("加载缓存文件失败: {path:?}, 错误: {e}");
                    // 删除损坏的缓存文件
                    let _ = std::fs::remove_file(&path);
                } else {
                    loaded_count += 1;
                }
            }
        }

        info!("加载了 {loaded_count} 个现有缓存文件");

        Ok(())
    }

    /// 加载单个缓存文件
    fn load_cache_file(&mut self, path: &Path) -> Result<()> {
        let mut file = File::open(path).context("打开缓存文件失败")?;

        // 读取元数据
        let mut metadata_bytes = [0u8; CacheMetadata::METADATA_SIZE];
        file.read_exact(&mut metadata_bytes)
            .context("读取元数据失败")?;

        let metadata =
            CacheMetadata::from_fixed_bytes(&metadata_bytes).context("解析元数据失败")?;

        // 检查是否过期
        if metadata.is_expired() {
            debug!("加载时发现过期缓存文件: {path:?}");
            std::fs::remove_file(path).context("删除过期缓存文件失败")?;
            return Ok(());
        }

        // 获取文件大小
        let file_size = file.metadata().context("获取文件元数据失败")?.len();

        // 创建缓存条目
        let cache_entry = CacheEntry::new(metadata.clone(), path.to_path_buf());
        let name_hash = Self::calculate_name_hash(&metadata.original_name);

        self.cache_index
            .insert(metadata.file_hash.clone(), cache_entry);
        self.name_hash_index
            .insert(name_hash, metadata.file_hash.clone());
        self.current_cache_size += file_size;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tempfile::tempdir;

    #[test]
    fn test_cache_metadata_serialization() {
        let metadata = CacheMetadata::new(
            "test.txt".to_string(),
            1024,
            512,
            "abc123".to_string(),
            3600,
        );

        let bytes = metadata.to_fixed_bytes().unwrap();
        let deserialized = CacheMetadata::from_fixed_bytes(&bytes).unwrap();

        assert_eq!(metadata.original_name, deserialized.original_name);
        assert_eq!(metadata.original_size, deserialized.original_size);
        assert_eq!(metadata.file_hash, deserialized.file_hash);
    }

    #[test]
    fn test_name_hash_calculation() {
        let name = "test_file.txt";
        let hash1 = GatewayCache::calculate_name_hash(name);
        let hash2 = GatewayCache::calculate_name_hash(name);

        assert_eq!(hash1, hash2);
        assert_eq!(hash1.len(), 64); // SHA256 hex length
    }

    #[test]
    fn test_cache_file_operations() -> Result<()> {
        let temp_dir = tempdir()?;
        let mut cache = GatewayCache::new(temp_dir.path(), 3600, 1024 * 1024)?;

        let test_data = b"Hello, World!";
        let file_hash = cache.cache_file("test.txt", test_data, None)?;

        let (retrieved_data, metadata) = cache.get_cached_file(&file_hash)?.unwrap();

        assert_eq!(retrieved_data, test_data);
        assert_eq!(metadata.original_name, "test.txt");
        assert_eq!(metadata.original_size, test_data.len() as u64);

        Ok(())
    }

    #[test]
    fn test_cache_expiration() -> Result<()> {
        let temp_dir = tempdir()?;
        let mut cache = GatewayCache::new(temp_dir.path(), 1, 1024 * 1024)?;

        let test_data = b"Hello, World!";
        let file_hash = cache.cache_file("test.txt", test_data, Some(0))?; // 立即过期

        std::thread::sleep(Duration::from_millis(100));

        let result = cache.get_cached_file(&file_hash)?;
        assert!(result.is_none());

        Ok(())
    }

    #[test]
    fn test_name_hash_list() -> Result<()> {
        let temp_dir = tempdir()?;
        let mut cache = GatewayCache::new(temp_dir.path(), 3600, 1024 * 1024)?;

        cache.cache_file("file1.txt", b"data1", None)?;
        cache.cache_file("file2.txt", b"data2", None)?;

        let hash_list = cache.get_name_hash_list();
        assert_eq!(hash_list.len(), 2);

        Ok(())
    }

    #[test]
    fn test_cache_cleanup() -> Result<()> {
        let temp_dir = tempdir()?;
        let mut cache = GatewayCache::new(temp_dir.path(), 1, 1024 * 1024)?;

        cache.cache_file("test1.txt", b"data1", Some(0))?; // 立即过期
        cache.cache_file("test2.txt", b"data2", None)?; // 不过期

        std::thread::sleep(Duration::from_millis(100));

        let cleaned = cache.cleanup_expired()?;
        assert_eq!(cleaned, 1);
        assert_eq!(cache.cache_index.len(), 1);

        Ok(())
    }
}
