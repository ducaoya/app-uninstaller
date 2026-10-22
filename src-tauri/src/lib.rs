use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

mod apps;
mod clean;
mod force;
mod launch;
mod model;
mod sweep;
mod uninstall;
mod util;

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
            uninstall_app,
            force_uninstall,
            clean_registry,
            clean_cache
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
