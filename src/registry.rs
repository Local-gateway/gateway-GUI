//! 网关注册表模块
//! 
//! 管理网关的注册表，存储网络中其他网关的信息。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use chrono::{DateTime, Utc};
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
#[derive(Debug, Clone)]
pub struct Registry {
    /// 存储所有注册的网关条目
    entries: HashMap<Uuid, RegistryEntry>,
    /// 本网关的信息
    local_entry: RegistryEntry,
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
            entries: HashMap::new(),
            local_entry: RegistryEntry::new(local_name, local_address),
        }
    }

    /// 获取本网关信息
    pub fn local_entry(&self) -> &RegistryEntry {
        &self.local_entry
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
    pub fn add_or_update(&mut self, mut entry: RegistryEntry) -> bool {
        // 不添加自己
        if entry.id == self.local_entry.id {
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
    /// 如果找到返回条目的引用，否则返回 None
    pub fn get(&self, id: &Uuid) -> Option<&RegistryEntry> {
        self.entries.get(id)
    }

    /// 根据地址获取网关条目
    /// 
    /// # 参数
    /// 
    /// * `address` - 网关地址
    /// 
    /// # 返回值
    /// 
    /// 如果找到返回条目的引用，否则返回 None
    pub fn get_by_address(&self, address: &SocketAddr) -> Option<&RegistryEntry> {
        self.entries.values().find(|entry| entry.address == *address)
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
    pub fn remove(&mut self, id: &Uuid) -> bool {
        self.entries.remove(id).is_some()
    }

    /// 获取所有注册条目（不包括本网关）
    /// 
    /// # 返回值
    /// 
    /// 所有条目的向量
    pub fn all_entries(&self) -> Vec<RegistryEntry> {
        self.entries.values().cloned().collect()
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
            .values()
            .filter(|entry| entry.id != *exclude_id)
            .cloned()
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
    pub fn cleanup_expired(&mut self, timeout_seconds: i64) -> usize {
        let cutoff_time = Utc::now() - chrono::Duration::seconds(timeout_seconds);
        let expired_ids: Vec<Uuid> = self
            .entries
            .iter()
            .filter(|(_, entry)| entry.last_seen < cutoff_time)
            .map(|(id, _)| *id)
            .collect();

        for id in &expired_ids {
            self.entries.remove(id);
        }

        expired_ids.len()
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
        let mut registry = Registry::new("本地网关".to_string(), local_address);
        
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
        let mut registry = Registry::new("本地网关".to_string(), local_address);
        
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
        let mut registry = Registry::new("本地网关".to_string(), local_address);
        
        // 尝试添加自己
        let is_new = registry.add_or_update(registry.local_entry().clone());
        assert!(!is_new);
        assert_eq!(registry.size(), 0);
    }

    #[test]
    fn test_registry_get_by_address() {
        let local_address = create_test_address(55555);
        let mut registry = Registry::new("本地网关".to_string(), local_address);
        
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
        let mut registry = Registry::new("本地网关".to_string(), local_address);
        
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
        let mut registry = Registry::new("本地网关".to_string(), local_address);
        
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
        let mut registry = Registry::new("本地网关".to_string(), local_address);
        
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
        let mut registry = Registry::new("本地网关".to_string(), local_address);
        
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