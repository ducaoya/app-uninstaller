use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
use winreg::{HKEY, RegKey};

use crate::model::AppInfo;

pub const UNINSTALL_PATH: &str = "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall";
pub const UNINSTALL_PATH_WOW64: &str =
    "SOFTWARE\\WOW6432Node\\Microsoft\\Windows\\CurrentVersion\\Uninstall";

pub type Root = (HKEY, &'static str, &'static str);

/// 三个卸载键根：(hive, hive 名, 子键路径)
pub fn uninstall_roots() -> Vec<Root> {
    vec![
        (HKEY_LOCAL_MACHINE, "HKLM", UNINSTALL_PATH),
        (HKEY_LOCAL_MACHINE, "WOW64", UNINSTALL_PATH_WOW64),
        (HKEY_CURRENT_USER, "HKCU", UNINSTALL_PATH),
    ]
}

/// 读取字符串值（REG_SZ / REG_EXPAND_SZ / REG_MULTI_SZ 均可）
pub fn read_string(key: &RegKey, name: &str) -> Option<String> {
    key.get_value::<String, _>(name).ok()
}

/// 扫描三个注册表卸载键，返回应用列表
pub fn list_apps() -> Vec<AppInfo> {
    let mut out: Vec<AppInfo> = Vec::new();
    for (hive, hive_name, path) in uninstall_roots() {
        let hk = match RegKey::predef(hive).open_subkey(path) {
            Ok(k) => k,
            Err(_) => continue,
        };
        for key_name in hk.enum_keys().flatten() {
            let sub = match hk.open_subkey(&key_name) {
                Ok(s) => s,
                Err(_) => continue,
            };
            // 跳过系统组件
            if sub.get_value::<u32, _>("SystemComponent").unwrap_or(0) == 1 {
                continue;
            }
            let Some(display) = read_string(&sub, "DisplayName")
                .filter(|d| !d.trim().is_empty())
            else {
                continue;
            };
            let uninstall_string =
                read_string(&sub, "UninstallString").unwrap_or_default();
            // 跳过无卸载串的 KB 补丁类条目
            if uninstall_string.trim().is_empty() && display.to_uppercase().starts_with("KB") {
                continue;
            }
            let mut is_msi =
                sub.get_value::<u32, _>("WindowsInstaller").unwrap_or(0) == 1;
            if uninstall_string.to_lowercase().contains("msiexec") {
                is_msi = true;
            }
            let id = format!("{}|{}|{}", hive_name, path, key_name);
            out.push(AppInfo {
                id,
                name: display,
                version: read_string(&sub, "DisplayVersion").unwrap_or_default(),
                publisher: read_string(&sub, "Publisher").unwrap_or_default(),
                install_location: read_string(&sub, "InstallLocation").unwrap_or_default(),
                uninstall_string,
                size_kb: sub.get_value::<u32, _>("EstimatedSize").unwrap_or(0) as u64,
                install_date: read_string(&sub, "InstallDate").unwrap_or_default(),
                is_msi,
            });
        }
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}
