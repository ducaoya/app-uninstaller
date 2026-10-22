use std::io;
use std::path::{Path, PathBuf};

use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
use winreg::{HKEY, RegKey};

use crate::model::{log_step, Logger, Report, Step};

/// 归一化：小写 + 压缩空白
pub fn normalize(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

/// 环境变量目录
pub fn env_dir(var: &str) -> Option<PathBuf> {
    std::env::var_os(var).map(PathBuf::from)
}

/// 判断尾部是否是纯版本号样式（" 2.1.0"、"v3" 等）
pub fn version_like(remainder: &str) -> bool {
    let r = remainder.trim();
    if r.is_empty() {
        return true;
    }
    let r = r.trim_start_matches(['v', 'V']);
    !r.is_empty() && r.chars().all(|c| c.is_ascii_digit() || c == '.' || c == ' ')
}

/// 系统保护路径（不删除）
pub fn is_protected(path: &Path) -> bool {
    let norm = path.to_string_lossy().trim_end_matches('\\').to_lowercase();
    if norm.len() <= 3 {
        return true; // 盘根
    }
    let mut deny: Vec<String> = vec![
        r"c:\windows".into(),
        r"c:\program files".into(),
        r"c:\program files (x86)".into(),
        r"c:\programdata".into(),
        r"c:\users".into(),
        r"c:\users\public".into(),
    ];
    for var in ["USERPROFILE", "LOCALAPPDATA", "APPDATA", "ProgramData"] {
        if let Some(d) = env_dir(var) {
            deny.push(d.to_string_lossy().trim_end_matches('\\').to_lowercase());
        }
    }
    deny.iter().any(|d| norm == *d)
}

/// 回收站删除（文件或目录）
pub fn recycle(path: &Path, action: &str, report: &mut Report, log: Logger) {
    let target = path.to_string_lossy().to_string();
    let step = if !path.exists() {
        Step::not_found(action, &target)
    } else {
        match trash::delete(path) {
            Ok(()) => Step::done(action, &target),
            Err(e) => Step::error(action, &target, format!("移入回收站失败: {e}")),
        }
    };
    log_step(log, &step);
    report.steps.push(step);
}

/// 直接删除目录（强制卸载用）
pub fn delete_dir_now(path: &Path, action: &str, report: &mut Report, log: Logger) {
    let target = path.to_string_lossy().to_string();
    let step = if !path.exists() {
        Step::not_found(action, &target)
    } else {
        match std::fs::remove_dir_all(path) {
            Ok(()) => Step::done(action, &target),
            Err(e) => Step::error(action, &target, format!("删除失败: {e}")),
        }
    };
    log_step(log, &step);
    report.steps.push(step);
}

/// 注册表键是否存在
pub fn key_exists(hive: HKEY, path: &str, name: &str) -> bool {
    match RegKey::predef(hive).open_subkey(format!("{path}\\{name}")) {
        Ok(_) => true,
        Err(e) if e.kind() == io::ErrorKind::NotFound => false,
        Err(_) => true, // 权限不足等按存在处理
    }
}

/// 递归删除注册表键（父路径 + 键名）
pub fn delete_key_tree(
    hive: HKEY,
    hive_label: &str,
    parent: &str,
    name: &str,
    action: &str,
    report: &mut Report,
    log: Logger,
) {
    let rel = format!("{parent}\\{name}");
    let full = format!("{hive_label}\\{rel}");
    let step = match RegKey::predef(hive).delete_subkey_all(&rel) {
        Ok(()) => Step::done(action, &full),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Step::not_found(action, &full),
        Err(e) => Step::error(action, &full, format!("删除失败: {e}")),
    };
    log_step(log, &step);
    report.steps.push(step);
}

/// AppData 下可能的残留目录（名称 / 发布者精确匹配）
pub fn appdata_candidates(name: &str, publisher: &str) -> Vec<PathBuf> {
    let bases: Vec<Option<PathBuf>> = vec![
        env_dir("LOCALAPPDATA"),
        env_dir("APPDATA"),
        env_dir("ProgramData"),
    ];
    let mut out = Vec::new();
    for base in bases.into_iter().flatten() {
        for label in [name, publisher] {
            let label = label.trim();
            if label.is_empty() || normalize(label).is_empty() {
                continue;
            }
            let dir = base.join(label);
            if dir.exists() && !out.contains(&dir) {
                out.push(dir);
            }
        }
    }
    out
}

/// 快捷方式搜索根目录
pub fn shortcut_roots() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(pd) = env_dir("ProgramData") {
        out.push(pd.join(r"Microsoft\Windows\Start Menu\Programs"));
    }
    if let Some(ad) = env_dir("APPDATA") {
        out.push(ad.join(r"Microsoft\Windows\Start Menu\Programs"));
    }
    if let Some(pubdir) = env_dir("PUBLIC") {
        out.push(pubdir.join("Desktop"));
    }
    if let Some(up) = env_dir("USERPROFILE") {
        out.push(up.join("Desktop"));
    }
    out
}

/// 已知共享厂商键，禁止整键删除
pub fn publisher_denied(publisher: &str) -> bool {
    const DENY: &[&str] = &[
        "microsoft",
        "microsoft corporation",
        "windows",
        "intel",
        "nvidia",
        "amd",
        "advanced micro devices",
        "google",
        "google llc",
        "apple",
        "adobe",
    ];
    let p = normalize(publisher);
    DENY.contains(&p.as_str())
}

/// 厂商/应用名注册表键候选：(hive, hive_label, 父路径, 键名)
pub fn publisher_key_candidates(
    publisher: &str,
    app_name: &str,
) -> Vec<(HKEY, &'static str, String, String)> {
    let mut out = Vec::new();
    for label in [publisher, app_name] {
        let label = label.trim();
        if label.is_empty() || publisher_denied(label) {
            continue;
        }
        for (hive, hive_name, prefix) in [
            (HKEY_LOCAL_MACHINE, "HKLM", "SOFTWARE"),
            (HKEY_LOCAL_MACHINE, "WOW64", r"SOFTWARE\WOW6432Node"),
            (HKEY_CURRENT_USER, "HKCU", "SOFTWARE"),
        ] {
            out.push((hive, hive_name, prefix.to_string(), label.to_string()));
        }
    }
    out
}
