//! 传输时自动zstd压缩/解压缩模块
//!
//! 提供智能压缩策略，根据数据大小自动决定是否压缩，
//! 以及多种压缩级别配置，为网络传输提供最优性能。

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use zstd;

/// 压缩配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionConfig {
    /// 压缩级别 (1-22, 默认3)
    pub level: i32,
    /// 最小压缩阈值，小于此大小的数据不压缩 (字节)
    pub min_compress_size: usize,
    /// 最大压缩阈值，大于此大小的数据分块压缩 (字节)
    pub max_chunk_size: usize,
    /// 是否启用字典压缩
    pub enable_dict: bool,
}

impl Default for CompressionConfig {
    fn default() -> Self {
        Self {
            level: 3,                    // 平衡压缩比和速度
            min_compress_size: 128,      // 小于128字节不压缩
            max_chunk_size: 1024 * 1024, // 1MB分块
            enable_dict: false,          // 暂不启用字典
        }
    }
}

/// 压缩统计信息
#[derive(Debug, Default, Clone)]
pub struct CompressionStats {
    /// 压缩次数
    pub compress_count: Arc<AtomicU64>,
    /// 解压次数
    pub decompress_count: Arc<AtomicU64>,
    /// 原始数据总大小
    pub total_raw_bytes: Arc<AtomicUsize>,
    /// 压缩后数据总大小
    pub total_compressed_bytes: Arc<AtomicUsize>,
    /// 压缩错误次数
    pub compress_errors: Arc<AtomicU64>,
    /// 解压错误次数
    pub decompress_errors: Arc<AtomicU64>,
}

impl CompressionStats {
    /// 创建新的压缩统计
    pub fn new() -> Self {
        Self::default()
    }

    /// 记录压缩操作
    pub fn record_compression(&self, raw_size: usize, compressed_size: usize) {
        self.compress_count.fetch_add(1, Ordering::Relaxed);
        self.total_raw_bytes.fetch_add(raw_size, Ordering::Relaxed);
        self.total_compressed_bytes
            .fetch_add(compressed_size, Ordering::Relaxed);
    }

    /// 记录解压操作
    pub fn record_decompression(&self, compressed_size: usize, raw_size: usize) {
        self.decompress_count.fetch_add(1, Ordering::Relaxed);
        self.total_compressed_bytes
            .fetch_add(compressed_size, Ordering::Relaxed);
        self.total_raw_bytes.fetch_add(raw_size, Ordering::Relaxed);
    }

    /// 记录压缩错误
    pub fn record_compress_error(&self) {
        self.compress_errors.fetch_add(1, Ordering::Relaxed);
    }

    /// 记录解压错误
    pub fn record_decompress_error(&self) {
        self.decompress_errors.fetch_add(1, Ordering::Relaxed);
    }

    /// 获取压缩比
    pub fn compression_ratio(&self) -> f64 {
        let raw = self.total_raw_bytes.load(Ordering::Relaxed);
        let compressed = self.total_compressed_bytes.load(Ordering::Relaxed);

        if raw == 0 {
            1.0
        } else {
            compressed as f64 / raw as f64
        }
    }

    /// 获取统计快照
    pub fn snapshot(&self) -> CompressionStatsSnapshot {
        CompressionStatsSnapshot {
            compress_count: self.compress_count.load(Ordering::Relaxed),
            decompress_count: self.decompress_count.load(Ordering::Relaxed),
            total_raw_bytes: self.total_raw_bytes.load(Ordering::Relaxed),
            total_compressed_bytes: self.total_compressed_bytes.load(Ordering::Relaxed),
            compress_errors: self.compress_errors.load(Ordering::Relaxed),
            decompress_errors: self.decompress_errors.load(Ordering::Relaxed),
            compression_ratio: self.compression_ratio(),
        }
    }
}

/// 压缩统计快照
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionStatsSnapshot {
    pub compress_count: u64,
    pub decompress_count: u64,
    pub total_raw_bytes: usize,
    pub total_compressed_bytes: usize,
    pub compress_errors: u64,
    pub decompress_errors: u64,
    pub compression_ratio: f64,
}

/// 压缩数据标识
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionFlag {
    /// 未压缩
    None = 0,
    /// Zstd压缩
    Zstd = 1,
}

impl From<u8> for CompressionFlag {
    fn from(value: u8) -> Self {
        match value {
            1 => CompressionFlag::Zstd,
            _ => CompressionFlag::None,
        }
    }
}

impl From<CompressionFlag> for u8 {
    fn from(flag: CompressionFlag) -> Self {
        flag as u8
    }
}

/// zstd压缩管理器
#[derive(Debug)]
pub struct CompressionManager {
    config: CompressionConfig,
    stats: CompressionStats,
}

impl CompressionManager {
    /// 创建新的压缩管理器
    pub fn new(config: CompressionConfig) -> Self {
        Self {
            config,
            stats: CompressionStats::new(),
        }
    }

    /// 获取配置
    pub fn config(&self) -> &CompressionConfig {
        &self.config
    }

    /// 获取统计信息
    pub fn stats(&self) -> &CompressionStats {
        &self.stats
    }

    /// 更新配置
    pub fn update_config(&mut self, config: CompressionConfig) {
        self.config = config;
    }

    /// 智能压缩数据
    ///
    /// 根据数据大小自动决定是否压缩，并添加压缩标识头
    pub fn compress(&self, data: &[u8]) -> Result<Vec<u8>> {
        // 小于阈值的数据不压缩
        if data.len() < self.config.min_compress_size {
            let mut result = Vec::with_capacity(data.len() + 1);
            result.push(CompressionFlag::None.into());
            result.extend_from_slice(data);
            return Ok(result);
        }

        // 尝试压缩
        match self.compress_zstd(data) {
            Ok(compressed) => {
                // 检查压缩效果，如果压缩后反而更大，则不压缩
                if compressed.len() >= data.len() {
                    let mut result = Vec::with_capacity(data.len() + 1);
                    result.push(CompressionFlag::None.into());
                    result.extend_from_slice(data);
                    Ok(result)
                } else {
                    self.stats.record_compression(data.len(), compressed.len());
                    let mut result = Vec::with_capacity(compressed.len() + 1);
                    result.push(CompressionFlag::Zstd.into());
                    result.extend_from_slice(&compressed);
                    Ok(result)
                }
            }
            Err(e) => {
                self.stats.record_compress_error();
                // 压缩失败，返回原始数据
                let mut result = Vec::with_capacity(data.len() + 1);
                result.push(CompressionFlag::None.into());
                result.extend_from_slice(data);
                log::warn!("压缩失败，使用原始数据: {e}");
                Ok(result)
            }
        }
    }

    /// zstd压缩
    fn compress_zstd(&self, data: &[u8]) -> Result<Vec<u8>> {
        zstd::bulk::compress(data, self.config.level).context("zstd压缩失败")
    }

    /// 解压数据
    ///
    /// 根据压缩标识头自动选择解压方法
    pub fn decompress(&self, data: &[u8]) -> Result<Vec<u8>> {
        if data.is_empty() {
            return Ok(Vec::new());
        }

        let flag = CompressionFlag::from(data[0]);
        let payload = &data[1..];

        match flag {
            CompressionFlag::None => {
                // 未压缩数据，直接返回
                Ok(payload.to_vec())
            }
            CompressionFlag::Zstd => {
                // zstd压缩数据，需要解压
                match self.decompress_zstd(payload) {
                    Ok(decompressed) => {
                        self.stats
                            .record_decompression(payload.len(), decompressed.len());
                        Ok(decompressed)
                    }
                    Err(e) => {
                        self.stats.record_decompress_error();
                        Err(e)
                    }
                }
            }
        }
    }

    /// zstd解压
    fn decompress_zstd(&self, data: &[u8]) -> Result<Vec<u8>> {
        zstd::bulk::decompress(data, self.config.max_chunk_size).context("zstd解压失败")
    }

    /// 分块压缩大数据
    pub fn compress_chunks(&self, data: &[u8]) -> Result<Vec<Vec<u8>>> {
        if data.len() <= self.config.max_chunk_size {
            return Ok(vec![self.compress(data)?]);
        }

        let mut chunks = Vec::new();
        for chunk in data.chunks(self.config.max_chunk_size) {
            chunks.push(self.compress(chunk)?);
        }
        Ok(chunks)
    }

    /// 分块解压大数据
    pub fn decompress_chunks(&self, chunks: &[Vec<u8>]) -> Result<Vec<u8>> {
        let mut result = Vec::new();
        for chunk in chunks {
            let decompressed = self.decompress(chunk)?;
            result.extend_from_slice(&decompressed);
        }
        Ok(result)
    }

    /// 重置统计信息
    pub fn reset_stats(&self) {
        self.stats.compress_count.store(0, Ordering::Relaxed);
        self.stats.decompress_count.store(0, Ordering::Relaxed);
        self.stats.total_raw_bytes.store(0, Ordering::Relaxed);
        self.stats
            .total_compressed_bytes
            .store(0, Ordering::Relaxed);
        self.stats.compress_errors.store(0, Ordering::Relaxed);
        self.stats.decompress_errors.store(0, Ordering::Relaxed);
    }
}

impl Default for CompressionManager {
    fn default() -> Self {
        Self::new(CompressionConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compression_config_default() {
        let config = CompressionConfig::default();
        assert_eq!(config.level, 3);
        assert_eq!(config.min_compress_size, 128);
        assert_eq!(config.max_chunk_size, 1024 * 1024);
        assert!(!config.enable_dict);
    }

    #[test]
    fn test_compression_flag_conversion() {
        assert_eq!(CompressionFlag::from(0u8), CompressionFlag::None);
        assert_eq!(CompressionFlag::from(1u8), CompressionFlag::Zstd);
        assert_eq!(CompressionFlag::from(255u8), CompressionFlag::None); // 未知标识归为None

        assert_eq!(u8::from(CompressionFlag::None), 0u8);
        assert_eq!(u8::from(CompressionFlag::Zstd), 1u8);
    }

    #[test]
    fn test_compression_manager_creation() {
        let manager = CompressionManager::default();
        assert_eq!(manager.config().level, 3);

        let custom_config = CompressionConfig {
            level: 5,
            min_compress_size: 256,
            max_chunk_size: 512 * 1024,
            enable_dict: true,
        };
        let manager2 = CompressionManager::new(custom_config);
        assert_eq!(manager2.config().level, 5);
        assert_eq!(manager2.config().min_compress_size, 256);
    }

    #[test]
    fn test_small_data_no_compression() {
        let manager = CompressionManager::default();
        let small_data = b"hello"; // 5字节，小于默认阈值128

        let compressed = manager.compress(small_data).unwrap();
        assert_eq!(compressed[0], u8::from(CompressionFlag::None));
        assert_eq!(&compressed[1..], small_data);

        let decompressed = manager.decompress(&compressed).unwrap();
        assert_eq!(decompressed, small_data);
    }

    #[test]
    fn test_large_data_compression() {
        let manager = CompressionManager::default();
        let large_data = "a".repeat(500).into_bytes(); // 500字节，大于阈值

        let compressed = manager.compress(&large_data).unwrap();
        assert_eq!(compressed[0], u8::from(CompressionFlag::Zstd));
        assert!(compressed.len() < large_data.len() + 1); // 应该有压缩效果

        let decompressed = manager.decompress(&compressed).unwrap();
        assert_eq!(decompressed, large_data);
    }

    #[test]
    fn test_incompressible_data() {
        let manager = CompressionManager::default();
        // 随机数据通常压缩效果不好
        let random_data: Vec<u8> = (0..200).map(|i| (i * 17 + 7) as u8).collect();

        let compressed = manager.compress(&random_data).unwrap();
        let decompressed = manager.decompress(&compressed).unwrap();
        assert_eq!(decompressed, random_data);
    }

    #[test]
    fn test_empty_data() {
        let manager = CompressionManager::default();
        let empty_data = b"";

        let compressed = manager.compress(empty_data).unwrap();
        let decompressed = manager.decompress(&compressed).unwrap();
        assert_eq!(decompressed, empty_data);
    }

    #[test]
    fn test_compression_stats() {
        let manager = CompressionManager::default();
        let data = "x".repeat(300).into_bytes(); // 确保会被压缩

        let stats_before = manager.stats().snapshot();
        assert_eq!(stats_before.compress_count, 0);

        let compressed = manager.compress(&data).unwrap();
        manager.decompress(&compressed).unwrap();

        let stats_after = manager.stats().snapshot();
        assert_eq!(stats_after.compress_count, 1);
        assert_eq!(stats_after.decompress_count, 1);
        assert!(stats_after.compression_ratio < 1.0); // 应该有压缩效果
    }

    #[test]
    fn test_chunk_compression() {
        let manager = CompressionManager::new(CompressionConfig {
            max_chunk_size: 100, // 小的分块大小用于测试
            ..CompressionConfig::default()
        });

        let large_data = "x".repeat(250).into_bytes(); // 超过分块大小

        let chunks = manager.compress_chunks(&large_data).unwrap();
        assert!(chunks.len() > 1); // 应该被分成多块

        let decompressed = manager.decompress_chunks(&chunks).unwrap();
        assert_eq!(decompressed, large_data);
    }

    #[test]
    fn test_stats_reset() {
        let manager = CompressionManager::default();
        let data = "x".repeat(300).into_bytes();

        manager.compress(&data).unwrap();
        assert!(manager.stats().snapshot().compress_count > 0);

        manager.reset_stats();
        let stats = manager.stats().snapshot();
        assert_eq!(stats.compress_count, 0);
        assert_eq!(stats.total_raw_bytes, 0);
    }

    #[test]
    fn test_config_update() {
        let mut manager = CompressionManager::default();
        assert_eq!(manager.config().level, 3);

        let new_config = CompressionConfig {
            level: 6,
            ..CompressionConfig::default()
        };
        manager.update_config(new_config);
        assert_eq!(manager.config().level, 6);
    }
}
