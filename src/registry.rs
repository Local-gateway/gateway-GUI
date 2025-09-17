//! 网关注册表模块
//!
//! 管理网关的注册表，存储网络中其他网关的信息。
//! 使用lock-free数据结构实现高性能并发访问。

use atomic_refcell::AtomicRefCell;
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;
use uuid::Uuid;

/// 注册表条目
///
/// 存储网关的基本信息，包括名称、地址和最后更新时间。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RegistryEntry {
    /// 网关唯一标识
    pub id: Uuid,
    /// 网关名称
    pub name: String,
    /// 网关地址 (IP + 端口)
    pub address: SocketAddr,
    /// 最后更新时间
    pub last_seen: DateTime<Utc>,
}

impl RegistryEntry {
    /// 创建新的注册表条目
    ///
    /// # 参数
    ///
    /// * `name` - 网关名称
    /// * `address` - 网关地址
    ///
    /// # 返回值
    ///
    /// 新创建的注册表条目
    pub fn new(name: String, address: SocketAddr) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            address,
            last_seen: Utc::now(),
        }
    }

    /// 更新最后访问时间
    pub fn update_last_seen(&mut self) {
        self.last_seen = Utc::now();
    }
}

/// 网关注册表
///
/// 管理网络中所有已知网关的注册信息。
/// 使用DashMap实现lock-free并发访问。
#[derive(Debug)]
pub struct Registry {
    /// 存储所有注册的网关条目 (lock-free)
    entries: Arc<DashMap<Uuid, RegistryEntry>>,
    /// 本网关的信息
    local_entry: Arc<AtomicRefCell<RegistryEntry>>,
}

impl Clone for Registry {
    fn clone(&self) -> Self {
        Self {
            entries: Arc::clone(&self.entries),
            local_entry: Arc::clone(&self.local_entry),
        }
    }
}

impl Registry {
    /// 创建新的注册表
    ///
    /// # 参数
    ///
    /// * `local_name` - 本网关的名称
    /// * `local_address` - 本网关的地址
    ///
    /// # 返回值
    ///
    /// 新创建的注册表实例
    pub fn new(local_name: String, local_address: SocketAddr) -> Self {
        Self {
            entries: Arc::new(DashMap::new()),
            local_entry: Arc::new(AtomicRefCell::new(RegistryEntry::new(
                local_name,
                local_address,
            ))),
        }
    }

    /// 获取本网关信息
    pub fn local_entry(&self) -> RegistryEntry {
        self.local_entry.borrow().clone()
    }

    /// 添加或更新网关条目
    ///
    /// # 参数
    ///
    /// * `entry` - 要添加或更新的条目
    ///
    /// # 返回值
    ///
    /// 如果是新添加的条目返回 true，如果是更新现有条目返回 false
    pub fn add_or_update(&self, mut entry: RegistryEntry) -> bool {
        // 不添加自己
        let local_id = self.local_entry.borrow().id;
        if entry.id == local_id {
            return false;
        }

        entry.update_last_seen();
        let is_new = !self.entries.contains_key(&entry.id);
        self.entries.insert(entry.id, entry);
        is_new
    }

    /// 根据 ID 获取网关条目
    ///
    /// # 参数
    ///
    /// * `id` - 网关唯一标识
    ///
    /// # 返回值
    ///
    /// 如果找到返回条目的拷贝，否则返回 None
    pub fn get(&self, id: &Uuid) -> Option<RegistryEntry> {
        self.entries.get(id).map(|entry| entry.clone())
    }

    /// 根据地址获取网关条目
    ///
    /// # 参数
    ///
    /// * `address` - 网关地址
    ///
    /// # 返回值
    ///
    /// 如果找到返回条目的拷贝，否则返回 None
    pub fn get_by_address(&self, address: &SocketAddr) -> Option<RegistryEntry> {
        self.entries
            .iter()
            .find(|entry| entry.address == *address)
            .map(|entry| entry.clone())
    }

    /// 移除网关条目
    ///
    /// # 参数
    ///
    /// * `id` - 要移除的网关 ID
    ///
    /// # 返回值
    ///
    /// 如果条目存在并被移除返回 true，否则返回 false
    pub fn remove(&self, id: &Uuid) -> bool {
        self.entries.remove(id).is_some()
    }

    /// 获取所有注册条目（不包括本网关）
    ///
    /// # 返回值
    ///
    /// 所有条目的向量
    pub fn all_entries(&self) -> Vec<RegistryEntry> {
        self.entries.iter().map(|entry| entry.clone()).collect()
    }

    /// 获取除指定条目外的所有条目
    ///
    /// # 参数
    ///
    /// * `exclude_id` - 要排除的网关 ID
    ///
    /// # 返回值
    ///
    /// 过滤后的条目向量
    pub fn entries_except(&self, exclude_id: &Uuid) -> Vec<RegistryEntry> {
        self.entries
            .iter()
            .filter(|entry| entry.id != *exclude_id)
            .map(|entry| entry.clone())
            .collect()
    }

    /// 清理过期的条目
    ///
    /// 移除超过指定时间未更新的条目。
    ///
    /// # 参数
    ///
    /// * `timeout_seconds` - 超时秒数
    ///
    /// # 返回值
    ///
    /// 被清理的条目数量
    pub fn cleanup_expired(&self, timeout_seconds: i64) -> usize {
        let cutoff_time = Utc::now() - chrono::Duration::seconds(timeout_seconds);
        let expired_ids: Vec<Uuid> = self
            .entries
            .iter()
            .filter(|entry| entry.last_seen < cutoff_time)
            .map(|entry| entry.id)
            .collect();

        let mut count = 0;
        for id in &expired_ids {
            if self.entries.remove(id).is_some() {
                count += 1;
            }
        }

        count
    }

    /// 获取注册表大小
    ///
    /// # 返回值
    ///
    /// 注册表中的条目数量（不包括本网关）
    pub fn size(&self) -> usize {
        self.entries.len()
    }

    /// 检查注册表是否为空
    ///
    /// # 返回值
    ///
    /// 如果注册表为空返回 true，否则返回 false
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    // ============================================================================
    // 挂载点管理方法（为 Tauri API 添加）
    // ============================================================================

    /// 注册挂载点
    ///
    /// # 参数
    ///
    /// * `mount_id` - 挂载点 ID
    /// * `local_path` - 本地路径
    /// * `mount_name` - 挂载名称
    /// * `read_only` - 是否只读
    ///
    /// # 返回值
    ///
    /// 操作结果
    pub async fn register_mount_point(
        &self,
        _mount_id: String,
        local_path: std::path::PathBuf,
        mount_name: String,
        read_only: bool,
    ) -> anyhow::Result<()> {
        // 验证路径存在且是目录
        if !local_path.exists() {
            return Err(anyhow::anyhow!("路径不存在: {:?}", local_path));
        }

        if !local_path.is_dir() {
            return Err(anyhow::anyhow!("不是目录: {:?}", local_path));
        }

        // 这里可以添加实际的挂载点注册逻辑
        // 目前只是简单的验证
        log::info!(
            "注册挂载点: {mount_name} -> {local_path:?} (只读: {read_only})"
        );

        Ok(())
    }

    /// 取消注册挂载点
    ///
    /// # 参数
    ///
    /// * `mount_id` - 挂载点 ID
    ///
    /// # 返回值
    ///
    /// 操作结果
    pub async fn unregister_mount_point(&self, mount_id: &str) -> anyhow::Result<()> {
        // 这里可以添加实际的挂载点取消注册逻辑
        log::info!("取消注册挂载点: {mount_id}");
        Ok(())
    }

    /// 获取挂载点列表
    ///
    /// # 返回值
    ///
    /// 挂载点列表
    pub async fn get_mount_points(&self) -> anyhow::Result<Vec<crate::tauri_api::MountPoint>> {
        use chrono::Utc;
        use uuid::Uuid;

        // 这里应该从实际存储中获取挂载点
        // 目前返回示例数据
        let mount_points = vec![
            crate::tauri_api::MountPoint {
                id: Uuid::new_v4().to_string(),
                local_path: std::path::PathBuf::from("/tmp"),
                mount_name: "临时目录".to_string(),
                read_only: false,
                mount_time: Utc::now(),
                file_count: 0,
                total_size: 0,
            },
        ];

        Ok(mount_points)
    }

    /// 列出目录内容
    ///
    /// # 参数
    ///
    /// * `mount_id` - 挂载点 ID
    /// * `path` - 目录路径
    ///
    /// # 返回值
    ///
    /// 目录条目列表
    pub async fn list_directory(
        &self,
        _mount_id: &str,
        path: &str,
    ) -> anyhow::Result<Vec<crate::tauri_api::DirectoryEntry>> {
        use std::fs;
        use chrono::{DateTime, Utc};

        // 这里应该根据 mount_id 找到实际的挂载点
        // 目前简化实现
        let target_path = std::path::PathBuf::from(path);

        if !target_path.exists() {
            return Err(anyhow::anyhow!("路径不存在: {:?}", target_path));
        }

        if !target_path.is_dir() {
            return Err(anyhow::anyhow!("不是目录: {:?}", target_path));
        }

        let mut entries = Vec::new();
        let read_dir = fs::read_dir(&target_path)?;

        for entry in read_dir {
            let entry = entry?;
            let metadata = entry.metadata()?;
            let file_name = entry.file_name().to_string_lossy().to_string();
            let full_path = entry.path();

            let modified_time = metadata.modified()?
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs();
            let modified_time = DateTime::from_timestamp(modified_time as i64, 0)
                .unwrap_or_else(Utc::now);

            let created_time = metadata.created().ok()
                .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                .and_then(|duration| DateTime::from_timestamp(duration.as_secs() as i64, 0));

            entries.push(crate::tauri_api::DirectoryEntry {
                name: file_name.clone(),
                path: full_path.to_string_lossy().to_string(),
                is_directory: metadata.is_dir(),
                size: if metadata.is_file() { metadata.len() } else { 0 },
                modified_time,
                created_time,
                file_type: if metadata.is_dir() {
                    "directory".to_string()
                } else if metadata.is_file() {
                    // 简单的文件类型推断
                    if let Some(extension) = full_path.extension() {
                        extension.to_string_lossy().to_string()
                    } else {
                        "file".to_string()
                    }
                } else {
                    "other".to_string()
                },
            });
        }

        // 按名称排序，目录在前
        entries.sort_by(|a, b| {
            match (a.is_directory, b.is_directory) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.name.cmp(&b.name),
            }
        });

        Ok(entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};

    fn create_test_address(port: u16) -> SocketAddr {
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)), port)
    }

    #[test]
    fn test_registry_entry_creation() {
        let address = create_test_address(55555);
        let entry = RegistryEntry::new("测试网关".to_string(), address);

        assert_eq!(entry.name, "测试网关");
        assert_eq!(entry.address, address);
        assert!(entry.last_seen <= Utc::now());
    }

    #[test]
    fn test_registry_entry_update_last_seen() {
        let address = create_test_address(55555);
        let mut entry = RegistryEntry::new("测试网关".to_string(), address);
        let original_time = entry.last_seen;

        // 等待一小段时间以确保时间戳不同
        std::thread::sleep(std::time::Duration::from_millis(1));
        entry.update_last_seen();

        assert!(entry.last_seen > original_time);
    }

    #[test]
    fn test_registry_creation() {
        let address = create_test_address(55555);
        let registry = Registry::new("本地网关".to_string(), address);

        assert_eq!(registry.local_entry().name, "本地网关");
        assert_eq!(registry.local_entry().address, address);
        assert!(registry.is_empty());
        assert_eq!(registry.size(), 0);
    }

    #[test]
    fn test_registry_add_entry() {
        let local_address = create_test_address(55555);
        let registry = Registry::new("本地网关".to_string(), local_address);

        let remote_address = create_test_address(55556);
        let entry = RegistryEntry::new("远程网关".to_string(), remote_address);

        let is_new = registry.add_or_update(entry.clone());
        assert!(is_new);
        assert_eq!(registry.size(), 1);

        let retrieved = registry.get(&entry.id);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name, "远程网关");
    }

    #[test]
    fn test_registry_update_existing_entry() {
        let local_address = create_test_address(55555);
        let registry = Registry::new("本地网关".to_string(), local_address);

        let remote_address = create_test_address(55556);
        let entry = RegistryEntry::new("远程网关".to_string(), remote_address);
        let original_time = entry.last_seen;

        registry.add_or_update(entry.clone());

        // 等待一小段时间然后更新
        std::thread::sleep(std::time::Duration::from_millis(1));
        let is_new = registry.add_or_update(entry.clone());
        assert!(!is_new);
        assert_eq!(registry.size(), 1);

        let retrieved = registry.get(&entry.id).unwrap();
        assert!(retrieved.last_seen > original_time);
    }

    #[test]
    fn test_registry_prevent_self_registration() {
        let local_address = create_test_address(55555);
        let registry = Registry::new("本地网关".to_string(), local_address);

        // 尝试添加自己
        let is_new = registry.add_or_update(registry.local_entry().clone());
        assert!(!is_new);
        assert_eq!(registry.size(), 0);
    }

    #[test]
    fn test_registry_get_by_address() {
        let local_address = create_test_address(55555);
        let registry = Registry::new("本地网关".to_string(), local_address);

        let remote_address = create_test_address(55556);
        let entry = RegistryEntry::new("远程网关".to_string(), remote_address);

        registry.add_or_update(entry.clone());

        let retrieved = registry.get_by_address(&remote_address);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().id, entry.id);
    }

    #[test]
    fn test_registry_remove_entry() {
        let local_address = create_test_address(55555);
        let registry = Registry::new("本地网关".to_string(), local_address);

        let remote_address = create_test_address(55556);
        let entry = RegistryEntry::new("远程网关".to_string(), remote_address);

        registry.add_or_update(entry.clone());
        assert_eq!(registry.size(), 1);

        let removed = registry.remove(&entry.id);
        assert!(removed);
        assert_eq!(registry.size(), 0);

        // 尝试移除不存在的条目
        let removed_again = registry.remove(&entry.id);
        assert!(!removed_again);
    }

    #[test]
    fn test_registry_entries_except() {
        let local_address = create_test_address(55555);
        let registry = Registry::new("本地网关".to_string(), local_address);

        let entry1 = RegistryEntry::new("网关1".to_string(), create_test_address(55556));
        let entry2 = RegistryEntry::new("网关2".to_string(), create_test_address(55557));
        let entry3 = RegistryEntry::new("网关3".to_string(), create_test_address(55558));

        registry.add_or_update(entry1.clone());
        registry.add_or_update(entry2.clone());
        registry.add_or_update(entry3.clone());

        let entries_except_1 = registry.entries_except(&entry1.id);
        assert_eq!(entries_except_1.len(), 2);
        assert!(!entries_except_1.iter().any(|e| e.id == entry1.id));
    }

    #[test]
    fn test_registry_cleanup_expired() {
        let local_address = create_test_address(55555);
        let registry = Registry::new("本地网关".to_string(), local_address);

        // 添加一些条目
        let entry1 = RegistryEntry::new("网关1".to_string(), create_test_address(55556));
        let entry2 = RegistryEntry::new("网关2".to_string(), create_test_address(55557));

        // 手动创建一个过期的条目
        let mut old_entry = RegistryEntry::new("旧网关".to_string(), create_test_address(55558));
        old_entry.last_seen = chrono::Utc::now() - chrono::Duration::seconds(3600);

        registry.add_or_update(entry1);
        registry.add_or_update(entry2);
        registry.entries.insert(old_entry.id, old_entry);

        assert_eq!(registry.size(), 3);

        // 清理超过 1800 秒的条目
        let cleaned_count = registry.cleanup_expired(1800);
        assert_eq!(cleaned_count, 1);
        assert_eq!(registry.size(), 2);
    }

    #[test]
    fn test_registry_all_entries() {
        let local_address = create_test_address(55555);
        let registry = Registry::new("本地网关".to_string(), local_address);

        let entry1 = RegistryEntry::new("网关1".to_string(), create_test_address(55556));
        let entry2 = RegistryEntry::new("网关2".to_string(), create_test_address(55557));

        registry.add_or_update(entry1.clone());
        registry.add_or_update(entry2.clone());

        let all_entries = registry.all_entries();
        assert_eq!(all_entries.len(), 2);

        let ids: std::collections::HashSet<Uuid> = all_entries.iter().map(|e| e.id).collect();
        assert!(ids.contains(&entry1.id));
        assert!(ids.contains(&entry2.id));
    }
}
