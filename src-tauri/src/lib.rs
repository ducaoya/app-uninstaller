use std::collections::HashMap;
use std::os::windows::process::CommandExt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

mod apps;
mod clean;
mod force;
mod icons;
mod launch;
mod model;
mod sweep;
mod uninstall;
mod util;

use serde::Serialize;

use crate::model::{AppInfo, LogLine, Report};

/// 应用记录：AppInfo + 用于定位注册表键的信息
#[derive(Debug, Clone)]
pub struct Record {
    pub id: String,
    /// "HKLM" | "WOW64" | "HKCU"
    pub hive_name: String,
    /// 卸载根路径（不含键名）
    pub uninstall_root: String,
    /// 卸载键名
    pub key_name: String,
    pub app: AppInfo,
}

impl Record {
    /// id 格式："hive|uninstall_root|key_name"
    pub fn parse_id(id: &str) -> Option<(String, String, String)> {
        let mut it = id.splitn(3, '|');
        let hive = it.next()?.to_string();
        let root = it.next()?.to_string();
        let name = it.next()?.to_string();
        if hive.is_empty() || root.is_empty() || name.is_empty() {
            return None;
        }
        Some((hive, root, name))
    }
}

#[derive(Default)]
pub struct AppState {
    pub map: Mutex<HashMap<String, Record>>,
    pub cancelled: Arc<AtomicBool>,
}

pub fn get_record(state: &tauri::State<'_, AppState>, id: &str) -> Result<Record, String> {
    state
        .map
        .lock()
        .unwrap()
        .get(id)
        .cloned()
        .ok_or_else(|| "应用记录不存在，请先刷新列表".to_string())
}

/// 前端实时日志回调（分级）
pub fn make_log(app: &tauri::AppHandle) -> impl Fn(&str, String) + '_ {
    use tauri::Emitter;
    move |level: &str, text: String| {
        let _ = app.emit(
            "job-log",
            LogLine { level: level.to_string(), text },
        );
    }
}

/// 任务取消请求
#[tauri::command]
async fn cancel_job(state: tauri::State<'_, AppState>) -> Result<(), String> {
    state.cancelled.store(true, Ordering::SeqCst);
    Ok(())
}

/// GitHub 仓库（上传后改成实际 owner/repo 即可启用检查更新）
const UPDATE_REPO: &str = "ducaoya/app-uninstaller";

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub current: String,
    pub latest: String,
    pub has_update: bool,
    pub url: String,
    pub notes: String,
}

fn version_gt(a: &str, b: &str) -> bool {
    fn parts(s: &str) -> Vec<u64> {
        s.trim_start_matches('v')
            .split('.')
            .map(|p| {
                let digits: String = p.chars().take_while(|c| c.is_ascii_digit()).collect();
                digits.parse().unwrap_or(0)
            })
            .collect()
    }
    let (pa, pb) = (parts(a), parts(b));
    for i in 0..3 {
        let x = pa.get(i).copied().unwrap_or(0);
        let y = pb.get(i).copied().unwrap_or(0);
        if x != y {
            return x > y;
        }
    }
    false
}

/// 批量读取应用图标（PNG data URL）
#[tauri::command]
async fn load_icons(keys: Vec<String>) -> Result<std::collections::HashMap<String, String>, String> {
    tauri::async_runtime::spawn_blocking(move || icons::load(&keys))
        .await
        .map_err(|e| e.to_string())
}

/// 检查更新（GitHub Releases）
#[tauri::command]
async fn check_update(app: tauri::AppHandle) -> Result<UpdateInfo, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let url = format!("https://api.github.com/repos/{UPDATE_REPO}/releases/latest");
        // 显式接入 schannel TLS（ureq 的 native-tls 特性只是适配器，不会自动启用）
        let tls = native_tls::TlsConnector::new()
            .map_err(|e| format!("TLS 初始化失败：{e}"))?;
        let agent = ureq::AgentBuilder::new()
            .tls_connector(std::sync::Arc::new(tls))
            .build();
        let resp = agent
            .get(&url)
            .set("User-Agent", "app-uninstaller")
            .set("Accept", "application/vnd.github+json")
            .timeout(std::time::Duration::from_secs(15))
            .call()
            .map_err(|e| match e {
                ureq::Error::Status(404, _) => {
                    format!("仓库 {UPDATE_REPO} 尚未发布版本（上传 GitHub 后可用）")
                }
                other => format!("网络请求失败：{other}"),
            })?;
        let body: serde_json::Value = resp.into_json().map_err(|e| format!("解析响应失败：{e}"))?;
        let latest = body["tag_name"]
            .as_str()
            .unwrap_or("")
            .trim_start_matches('v')
            .to_string();
        let html_url = body["html_url"].as_str().unwrap_or_default().to_string();
        let notes = body["body"].as_str().unwrap_or_default().chars().take(300).collect();
        let current = app.package_info().version.to_string();
        let has_update = version_gt(&latest, &current);
        Ok(UpdateInfo { current, latest, has_update, url: html_url, notes })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// 打开外部链接（仅限 GitHub）
#[tauri::command]
async fn open_url(url: String) -> Result<(), String> {
    if !url.starts_with("https://github.com/") {
        return Err("仅允许打开 GitHub 链接".to_string());
    }
    std::process::Command::new("cmd")
        .args(["/c", "start", "", &url])
        .creation_flags(0x08000000) // CREATE_NO_WINDOW
        .spawn()
        .map_err(|e| format!("打开浏览器失败：{e}"))?
        .wait()
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// 扫描注册表列出所有应用
#[tauri::command]
async fn list_apps(state: tauri::State<'_, AppState>) -> Result<Vec<AppInfo>, String> {
    let list = tauri::async_runtime::spawn_blocking(apps::list_apps)
        .await
        .map_err(|e| e.to_string())?;
    let mut map = state.map.lock().unwrap();
    map.clear();
    for app in list.iter() {
        if let Some((hive, root, name)) = Record::parse_id(&app.id) {
            map.insert(
                app.id.clone(),
                Record {
                    id: app.id.clone(),
                    hive_name: hive,
                    uninstall_root: root,
                    key_name: name,
                    app: app.clone(),
                },
            );
        }
    }
    Ok(list)
}

/// 标准卸载：唤起系统卸载器，等待结束后扫描并清理残留
#[tauri::command]
async fn uninstall_app(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<Report, String> {
    let rec = get_record(&state, &id)?;
    let cancelled = state.cancelled.clone();
    tauri::async_runtime::spawn_blocking(move || {
        cancelled.store(false, Ordering::SeqCst);
        let log = make_log(&app);
        let cancel = || cancelled.load(Ordering::SeqCst);
        uninstall::run(&rec, &log, &cancel)
    })
    .await
    .map_err(|e| e.to_string())
}

/// 强制卸载：杀进程 + 直接删除文件/注册表 + 残留清理
#[tauri::command]
async fn force_uninstall(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<Report, String> {
    let rec = get_record(&state, &id)?;
    let cancelled = state.cancelled.clone();
    tauri::async_runtime::spawn_blocking(move || {
        cancelled.store(false, Ordering::SeqCst);
        let log = make_log(&app);
        let cancel = || cancelled.load(Ordering::SeqCst);
        force::run(&rec, &log, &cancel)
    })
    .await
    .map_err(|e| e.to_string())
}

/// 清除注册表：卸载键 + 厂商键
#[tauri::command]
async fn clean_registry(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<Report, String> {
    let rec = get_record(&state, &id)?;
    let cancelled = state.cancelled.clone();
    tauri::async_runtime::spawn_blocking(move || {
        cancelled.store(false, Ordering::SeqCst);
        let log = make_log(&app);
        let cancel = || cancelled.load(Ordering::SeqCst);
        clean::registry(&rec, &log, &cancel)
    })
    .await
    .map_err(|e| e.to_string())
}

/// 清除缓存文件夹：AppData\Local / Roaming / ProgramData 下同名目录（进回收站）
#[tauri::command]
async fn clean_cache(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<Report, String> {
    let rec = get_record(&state, &id)?;
    let cancelled = state.cancelled.clone();
    tauri::async_runtime::spawn_blocking(move || {
        cancelled.store(false, Ordering::SeqCst);
        let log = make_log(&app);
        let cancel = || cancelled.load(Ordering::SeqCst);
        clean::cache(&rec, &log, &cancel)
    })
    .await
    .map_err(|e| e.to_string())
}

pub fn run() {
    tauri::Builder::default()
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            list_apps,
            cancel_job,
            load_icons,
            uninstall_app,
            force_uninstall,
            clean_registry,
            clean_cache,
            check_update,
            open_url
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
