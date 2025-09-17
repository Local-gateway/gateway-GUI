//! 安全漏洞测试
//!
//! 测试路径遍历、文件访问控制和其他安全相关功能

use std::fs;
use tempfile::tempdir;

use wdic_gateway::security::{PathValidator, SecureFileReader, SearchResultFilter};
use wdic_gateway::udp_protocol::{DirectoryIndex, UdpBroadcastManager};

#[cfg(test)]
mod security_tests {
    use super::*;

    #[test]
    fn test_path_traversal_protection() {
        let temp_dir = tempdir().unwrap();
        let allowed_roots = vec![temp_dir.path().to_path_buf()];
        let validator = PathValidator::new(allowed_roots);

        // 创建测试文件
        let test_file = temp_dir.path().join("test.txt");
        fs::write(&test_file, b"safe content").unwrap();

        // 测试正常路径访问
        let normal_path = test_file.to_string_lossy().to_string();
        assert!(validator.validate_and_normalize(&normal_path).is_ok());

        // 测试路径遍历攻击
        let attack_patterns = vec![
            "../../../etc/passwd",
            "..\\..\\..\\windows\\system32\\config\\sam",
            "/etc/passwd",
            "C:\\Windows\\System32\\config\\SAM",
            "test.txt/../../etc/passwd",
            "./../../etc/passwd",
            "test/../../../etc/passwd",
        ];

        for attack_path in attack_patterns {
            let result = validator.validate_and_normalize(attack_path);
            assert!(
                result.is_err(),
                "路径遍历攻击应该被阻止: {attack_path}"
            );
        }
    }

    #[test]
    fn test_secure_file_reading() {
        let temp_dir = tempdir().unwrap();
        
        // 创建测试文件
        let safe_file = temp_dir.path().join("safe.txt");
        fs::write(&safe_file, b"safe content").unwrap();

        // 创建安全文件读取器
        let reader = SecureFileReader::new(
            vec![temp_dir.path().to_path_buf()],
            1024, // 1KB limit
        );

        // 测试正常文件读取
        let content = reader.read_file(&safe_file.to_string_lossy()).unwrap();
        assert_eq!(content, b"safe content");

        // 测试读取不存在的文件
        let non_existent = temp_dir.path().join("nonexistent.txt");
        assert!(reader.read_file(&non_existent.to_string_lossy()).is_err());

        // 测试读取目录（应该失败）
        assert!(reader.read_file(&temp_dir.path().to_string_lossy()).is_err());
    }

    #[test]
    fn test_file_size_limit() {
        let temp_dir = tempdir().unwrap();
        
        // 创建一个大文件
        let large_file = temp_dir.path().join("large.txt");
        let large_content = vec![b'x'; 2048]; // 2KB content
        fs::write(&large_file, &large_content).unwrap();

        // 创建有严格大小限制的文件读取器
        let reader = SecureFileReader::new(
            vec![temp_dir.path().to_path_buf()],
            1024, // 1KB limit
        );

        // 应该拒绝读取过大的文件
        assert!(reader.read_file(&large_file.to_string_lossy()).is_err());

        // 创建更宽松的文件读取器
        let relaxed_reader = SecureFileReader::new(
            vec![temp_dir.path().to_path_buf()],
            4096, // 4KB limit
        );

        // 现在应该可以读取
        assert!(relaxed_reader.read_file(&large_file.to_string_lossy()).is_ok());
    }

    #[test]
    fn test_search_result_filtering() {
        let filter = SearchResultFilter::new();

        let test_results = vec![
            "/home/user/document.pdf".to_string(),
            "/home/user/.ssh/id_rsa".to_string(),
            "/etc/passwd".to_string(),
            "/home/user/.gitignore".to_string(),
            "/home/user/.env".to_string(),
            "/home/user/public/readme.txt".to_string(),
            "/home/user/.aws/credentials".to_string(),
            "/home/user/config/database.conf".to_string(),
        ];

        let filtered = filter.filter_results(test_results);

        // 检查敏感文件被过滤掉
        assert!(!filtered.iter().any(|p| p.contains(".ssh")));
        assert!(!filtered.iter().any(|p| p.contains("passwd")));
        assert!(!filtered.iter().any(|p| p.contains(".env")));
        assert!(!filtered.iter().any(|p| p.contains(".aws")));
        assert!(!filtered.iter().any(|p| p.contains("config/")));

        // 检查安全文件被保留
        assert!(filtered.iter().any(|p| p.contains("document.pdf")));
        assert!(filtered.iter().any(|p| p.contains("readme.txt")));
        
        // .gitignore 应该被保留（是允许的隐藏文件）
        assert!(filtered.iter().any(|p| p.contains(".gitignore")));
    }

    #[tokio::test]
    async fn test_directory_index_security() {
        let temp_dir = tempdir().unwrap();
        
        // 创建测试目录结构
        let public_dir = temp_dir.path().join("public");
        fs::create_dir_all(&public_dir).unwrap();
        
        let safe_file = public_dir.join("safe.txt");
        fs::write(&safe_file, b"safe content").unwrap();

        // 创建隐藏目录（应该被跳过）
        let hidden_dir = temp_dir.path().join(".hidden");
        fs::create_dir_all(&hidden_dir).unwrap();
        let secret_file = hidden_dir.join("secret.txt");
        fs::write(&secret_file, b"secret content").unwrap();

        // 生成目录索引
        let index = DirectoryIndex::generate(&temp_dir.path().to_string_lossy()).unwrap();

        // 检查索引只包含公开文件，不包含隐藏文件
        let paths: Vec<String> = index.entries.iter().map(|e| e.path.clone()).collect();
        
        assert!(paths.iter().any(|p| p.contains("safe.txt")));
        assert!(!paths.iter().any(|p| p.contains("secret.txt")));
        assert!(!paths.iter().any(|p| p.contains(".hidden")));
    }

    #[tokio::test]
    async fn test_mount_directory_validation() {
        let temp_dir = tempdir().unwrap();
        
        // 创建测试目录
        let test_dir = temp_dir.path().join("test_mount");
        fs::create_dir_all(&test_dir).unwrap();

        #[allow(deprecated)]
        let manager = UdpBroadcastManager::new("127.0.0.1:0".parse().unwrap()).unwrap();

        // 测试正常挂载
        let result = manager
            .mount_directory("test".to_string(), test_dir.to_string_lossy().to_string())
            .await;
        assert!(result.is_ok());

        // 测试重复挂载（应该失败）
        let duplicate_result = manager
            .mount_directory("test".to_string(), test_dir.to_string_lossy().to_string())
            .await;
        assert!(duplicate_result.is_err());

        // 测试无效挂载点名称
        let invalid_names = vec![
            "".to_string(),
            "test/invalid".to_string(),
            "test\\invalid".to_string(),
            "test:invalid".to_string(),
            "test<invalid".to_string(),
            "test>invalid".to_string(),
            "test|invalid".to_string(),
            "test?invalid".to_string(),
            "test*invalid".to_string(),
        ];

        for invalid_name in invalid_names {
            let result = manager
                .mount_directory(invalid_name.clone(), test_dir.to_string_lossy().to_string())
                .await;
            assert!(
                result.is_err(),
                "无效挂载点名称应该被拒绝: {invalid_name}"
            );
        }

        // 测试挂载不存在的目录
        let non_existent_dir = temp_dir.path().join("nonexistent");
        let result = manager
            .mount_directory("invalid".to_string(), non_existent_dir.to_string_lossy().to_string())
            .await;
        assert!(result.is_err());

        // 测试挂载文件而不是目录
        let test_file = temp_dir.path().join("file.txt");
        fs::write(&test_file, b"content").unwrap();
        let result = manager
            .mount_directory("file_mount".to_string(), test_file.to_string_lossy().to_string())
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_secure_file_access() {
        let temp_dir = tempdir().unwrap();
        
        // 创建测试目录和文件
        let mount_dir = temp_dir.path().join("mounted");
        fs::create_dir_all(&mount_dir).unwrap();
        
        let safe_file = mount_dir.join("safe.txt");
        fs::write(&safe_file, b"safe content").unwrap();

        // 创建外部文件（不应该能访问）
        let external_file = temp_dir.path().join("external.txt");
        fs::write(&external_file, b"external content").unwrap();

        #[allow(deprecated)]
        let manager = UdpBroadcastManager::new("127.0.0.1:0".parse().unwrap()).unwrap();
        
        // 挂载目录
        manager
            .mount_directory("test".to_string(), mount_dir.to_string_lossy().to_string())
            .await
            .unwrap();

        // 测试访问挂载目录内的文件（应该成功）
        let result = manager.read_file(&safe_file.to_string_lossy()).await;
        assert!(result.is_ok());

        // 测试访问外部文件（应该失败）
        let external_result = manager.read_file(&external_file.to_string_lossy()).await;
        assert!(external_result.is_err());

        // 测试路径遍历攻击
        let attack_path = format!("{}/../external.txt", safe_file.display());
        let attack_result = manager.read_file(&attack_path).await;
        assert!(attack_result.is_err());
    }

    #[test]
    fn test_directory_depth_protection() {
        let validator = PathValidator::new(vec![]);

        // 测试正常深度的路径
        let normal_path = "/home/user/documents/projects/rust/project/src/main.rs";
        assert!(validator.validate_directory_depth(std::path::Path::new(normal_path)).is_ok());

        // 测试过深的路径
        let deep_path_components: Vec<String> = (0..50).map(|i| format!("dir{i}")).collect();
        let deep_path = format!("/{}", deep_path_components.join("/"));
        
        let result = validator.validate_directory_depth(std::path::Path::new(&deep_path));
        assert!(result.is_err(), "过深的路径应该被拒绝");
    }

    #[test]
    fn test_malicious_filename_protection() {
        let validator = PathValidator::new(vec![]);

        // 测试包含空字节的文件名
        let malicious_paths = vec![
            "normal_file\0.txt",
            "file\x01name.txt",
            "/path/to/file\0.txt",
        ];

        for malicious_path in malicious_paths {
            let result = validator.validate_and_normalize(malicious_path);
            assert!(
                result.is_err(),
                "包含恶意字符的路径应该被拒绝: {malicious_path}"
            );
        }
    }

    #[test]
    fn test_search_result_limit() {
        let filter = SearchResultFilter::new();

        // 创建超过限制数量的搜索结果
        let large_results: Vec<String> = (0..2000)
            .map(|i| format!("/home/user/file{i}.txt"))
            .collect();

        let filtered = filter.filter_results(large_results);

        // 检查结果数量被限制
        assert!(filtered.len() <= 1000, "搜索结果应该被限制在1000个以内");
    }
}