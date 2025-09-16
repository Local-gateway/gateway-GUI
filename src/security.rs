//! 安全模块
//!
//! 提供路径验证、文件访问控制和安全相关的实用函数

use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf, Component};

/// 最大目录遍历深度，防止无限递归
const MAX_DIRECTORY_DEPTH: usize = 32;

/// 最大文件路径长度，防止路径过长攻击
const MAX_PATH_LENGTH: usize = 4096;

/// 最大搜索结果数量，防止内存耗尽
const MAX_SEARCH_RESULTS: usize = 1000;

/// 安全路径验证器
pub struct PathValidator {
    /// 允许的根目录列表
    allowed_roots: Vec<PathBuf>,
}

impl PathValidator {
    /// 创建新的路径验证器
    ///
    /// # 参数
    ///
    /// * `allowed_roots` - 允许访问的根目录列表
    pub fn new(allowed_roots: Vec<PathBuf>) -> Self {
        Self { allowed_roots }
    }

    /// 验证和规范化文件路径
    ///
    /// # 参数
    ///
    /// * `path` - 要验证的路径
    ///
    /// # 返回值
    ///
    /// 规范化后的安全路径
    ///
    /// # 错误
    ///
    /// 如果路径包含路径遍历攻击或不在允许的根目录下，返回错误
    pub fn validate_and_normalize(&self, path: &str) -> Result<PathBuf> {
        // 检查路径长度
        if path.len() > MAX_PATH_LENGTH {
            return Err(anyhow!("路径过长: {} 字符，超过限制 {}", path.len(), MAX_PATH_LENGTH));
        }

        // 规范化路径
        let normalized = self.normalize_path(path)?;

        // 检查是否在允许的根目录下
        self.validate_within_allowed_roots(&normalized)?;

        Ok(normalized)
    }

    /// 规范化路径，移除 . 和 .. 组件
    fn normalize_path(&self, path: &str) -> Result<PathBuf> {
        let path = Path::new(path);
        let mut normalized = PathBuf::new();

        for component in path.components() {
            match component {
                Component::Normal(name) => {
                    // 检查文件名是否包含危险字符
                    let name_str = name.to_string_lossy();
                    if name_str.contains('\0') || name_str.contains('\x01') {
                        return Err(anyhow!("文件名包含非法字符: {}", name_str));
                    }
                    normalized.push(name);
                }
                Component::CurDir => {
                    // 忽略当前目录组件 "."
                    continue;
                }
                Component::ParentDir => {
                    // 处理父目录组件 ".."
                    if !normalized.pop() {
                        return Err(anyhow!("路径遍历攻击检测到: 尝试访问根目录之外的路径"));
                    }
                }
                Component::RootDir => {
                    // 保留根目录组件
                    normalized.push(component);
                }
                Component::Prefix(_) => {
                    // Windows 路径前缀，直接保留
                    normalized.push(component);
                }
            }
        }

        Ok(normalized)
    }

    /// 验证路径是否在允许的根目录下
    fn validate_within_allowed_roots(&self, path: &Path) -> Result<()> {
        if self.allowed_roots.is_empty() {
            return Ok(()); // 如果没有设置限制，则允许所有路径
        }

        for allowed_root in &self.allowed_roots {
            if path.starts_with(allowed_root) {
                return Ok(());
            }
        }

        Err(anyhow!(
            "路径访问被拒绝: {} 不在允许的根目录列表中",
            path.display()
        ))
    }

    /// 验证目录遍历深度
    ///
    /// # 参数
    ///
    /// * `path` - 要检查的路径
    ///
    /// # 返回值
    ///
    /// 如果深度超过限制返回错误
    pub fn validate_directory_depth(&self, path: &Path) -> Result<()> {
        let depth = path.components().count();
        if depth > MAX_DIRECTORY_DEPTH {
            return Err(anyhow!(
                "目录深度过深: {} 层，超过限制 {} 层",
                depth,
                MAX_DIRECTORY_DEPTH
            ));
        }
        Ok(())
    }
}

/// 安全的文件读取器
pub struct SecureFileReader {
    path_validator: PathValidator,
    /// 最大文件大小（字节）
    max_file_size: u64,
}

impl SecureFileReader {
    /// 创建新的安全文件读取器
    ///
    /// # 参数
    ///
    /// * `allowed_roots` - 允许访问的根目录列表
    /// * `max_file_size` - 最大文件大小限制
    pub fn new(allowed_roots: Vec<PathBuf>, max_file_size: u64) -> Self {
        Self {
            path_validator: PathValidator::new(allowed_roots),
            max_file_size,
        }
    }

    /// 安全地读取文件内容
    ///
    /// # 参数
    ///
    /// * `file_path` - 文件路径
    ///
    /// # 返回值
    ///
    /// 文件内容的字节数组
    ///
    /// # 错误
    ///
    /// 如果路径不安全或文件过大，返回错误
    pub fn read_file(&self, file_path: &str) -> Result<Vec<u8>> {
        // 验证和规范化路径
        let normalized_path = self.path_validator.validate_and_normalize(file_path)?;

        // 检查文件是否存在
        if !normalized_path.exists() {
            return Err(anyhow!("文件不存在: {}", normalized_path.display()));
        }

        // 检查是否为文件而不是目录
        if !normalized_path.is_file() {
            return Err(anyhow!("路径指向目录而非文件: {}", normalized_path.display()));
        }

        // 检查文件大小
        let metadata = std::fs::metadata(&normalized_path)
            .map_err(|e| anyhow!("无法获取文件元数据: {}", e))?;

        if metadata.len() > self.max_file_size {
            return Err(anyhow!(
                "文件过大: {} 字节，超过限制 {} 字节",
                metadata.len(),
                self.max_file_size
            ));
        }

        // 读取文件内容
        std::fs::read(&normalized_path)
            .map_err(|e| anyhow!("读取文件失败: {}", e))
    }
}

/// 搜索结果过滤器
pub struct SearchResultFilter {
    /// 最大结果数量
    max_results: usize,
    /// 禁止的路径模式
    forbidden_patterns: Vec<String>,
}

impl SearchResultFilter {
    /// 创建新的搜索结果过滤器
    pub fn new() -> Self {
        Self {
            max_results: MAX_SEARCH_RESULTS,
            forbidden_patterns: vec![
                "/.git/".to_string(),
                "/.ssh/".to_string(),
                "/etc/passwd".to_string(),
                "/etc/shadow".to_string(),
                "/.env".to_string(),
                "/id_rsa".to_string(),
                "/id_dsa".to_string(),
                "/.aws/".to_string(),
                "/config/".to_string(),
            ],
        }
    }

    /// 过滤搜索结果
    ///
    /// # 参数
    ///
    /// * `results` - 原始搜索结果
    ///
    /// # 返回值
    ///
    /// 过滤后的安全搜索结果
    pub fn filter_results(&self, results: Vec<String>) -> Vec<String> {
        results
            .into_iter()
            .filter(|path| self.is_path_allowed(path))
            .take(self.max_results)
            .collect()
    }

    /// 检查路径是否被允许
    fn is_path_allowed(&self, path: &str) -> bool {
        let path_lower = path.to_lowercase();
        
        // 检查是否包含禁止的模式
        for pattern in &self.forbidden_patterns {
            if path_lower.contains(pattern) {
                return false;
            }
        }

        // 检查是否为隐藏文件（以 . 开头的文件）
        if let Some(filename) = Path::new(path).file_name() {
            let filename_str = filename.to_string_lossy();
            if filename_str.starts_with('.') && filename_str.len() > 1 {
                // 允许一些常见的非敏感隐藏文件
                let allowed_hidden = [".gitignore", ".env.example", ".dockerignore"];
                if !allowed_hidden.iter().any(|&allowed| filename_str == allowed) {
                    return false;
                }
            }
        }

        true
    }
}

impl Default for SearchResultFilter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_path_validation() {
        let temp_dir = tempdir().unwrap();
        let allowed_roots = vec![temp_dir.path().to_path_buf()];
        let validator = PathValidator::new(allowed_roots);

        // 测试正常路径
        let normal_path = temp_dir.path().join("test.txt").to_string_lossy().to_string();
        assert!(validator.validate_and_normalize(&normal_path).is_ok());

        // 测试路径遍历攻击
        let traversal_path = format!("{}/../../../etc/passwd", temp_dir.path().display());
        assert!(validator.validate_and_normalize(&traversal_path).is_err());

        // 测试相对路径遍历
        assert!(validator.validate_and_normalize("../../../etc/passwd").is_err());
    }

    #[test]
    fn test_path_normalization() {
        let validator = PathValidator::new(vec![]);

        // 测试移除 . 组件
        let path_with_dots = "/home/user/./documents/./file.txt";
        let normalized = validator.normalize_path(path_with_dots).unwrap();
        assert_eq!(normalized, PathBuf::from("/home/user/documents/file.txt"));

        // 测试移除 .. 组件
        let path_with_parent = "/home/user/documents/../file.txt";
        let normalized = validator.normalize_path(path_with_parent).unwrap();
        assert_eq!(normalized, PathBuf::from("/home/user/file.txt"));
    }

    #[test]
    fn test_search_result_filter() {
        let filter = SearchResultFilter::new();

        let results = vec![
            "/home/user/document.txt".to_string(),
            "/home/user/.ssh/id_rsa".to_string(),
            "/etc/passwd".to_string(),
            "/home/user/.gitignore".to_string(),
            "/home/user/public/file.txt".to_string(),
        ];

        let filtered = filter.filter_results(results);
        
        // 应该保留安全的文件，包括允许的隐藏文件
        assert_eq!(filtered.len(), 3);
        assert!(filtered.contains(&"/home/user/document.txt".to_string()));
        assert!(filtered.contains(&"/home/user/public/file.txt".to_string()));
        assert!(filtered.contains(&"/home/user/.gitignore".to_string()));
        
        // 验证敏感文件被过滤掉
        assert!(!filtered.contains(&"/home/user/.ssh/id_rsa".to_string()));
        assert!(!filtered.contains(&"/etc/passwd".to_string()));
    }

    #[test]
    fn test_directory_depth_validation() {
        let validator = PathValidator::new(vec![]);

        // 测试正常深度
        let normal_path = Path::new("/home/user/documents/file.txt");
        assert!(validator.validate_directory_depth(normal_path).is_ok());

        // 测试过深的路径
        let deep_path_str = "/".to_owned() + &"a/".repeat(50);
        let deep_path = Path::new(&deep_path_str);
        assert!(validator.validate_directory_depth(deep_path).is_err());
    }

    #[test]
    fn test_secure_file_reader() {
        let temp_dir = tempdir().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        fs::write(&test_file, b"test content").unwrap();

        let reader = SecureFileReader::new(
            vec![temp_dir.path().to_path_buf()],
            1024 * 1024, // 1MB limit
        );

        // 测试正常文件读取
        let content = reader.read_file(&test_file.to_string_lossy()).unwrap();
        assert_eq!(content, b"test content");

        // 测试路径遍历攻击
        let malicious_path = format!("{}/../../../etc/passwd", test_file.display());
        assert!(reader.read_file(&malicious_path).is_err());
    }
}