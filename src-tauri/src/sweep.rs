use std::time::{Duration, Instant};

use winreg::HKEY;

use crate::model::Report;
use crate::util::{self};

/// 扫描并清理残留（标准卸载后 / 强制卸载末尾共用）
pub fn run(rec: &crate::Record, report: &mut Report, log: &dyn Fn(&str)) {
    let app = &rec.app;
    log("开始扫描残留…");

    // 1. 三个根下的同名卸载键
    for (hive, hive_name, root) in util_roots() {
        util::delete_key_tree(hive, hive_name, root, &rec.key_name, "删除卸载键", report);
    }

    // 2. 厂商 / 应用名注册表键
    for (hive, hive_name, prefix, label) in
        util::publisher_key_candidates(&app.publisher, &app.name)
    {
        util::delete_key_tree(hive, hive_name, &prefix, &label, "删除厂商键", report);
    }

    // 3. 安装目录残余（标准卸载后可能仍在；带保护路径检查）
    let install = app.install_location.trim().to_string();
    if !install.is_empty() {
        let dir = std::path::Path::new(&install);
        if dir.exists() && !util::is_protected(dir) {
            util::recycle(dir, "清理安装目录残余", report);
        }
    }

    // 4. AppData / ProgramData 同名目录（回收站）
    for dir in util::appdata_candidates(&app.name, &app.publisher) {
        util::recycle(&dir, "清理数据目录", report);
    }

    // 5. 快捷方式（回收站）
    let targets = shortcut_matches(&app.name);
    for root in util::shortcut_roots() {
        let mut found = Vec::new();
        walk_lnks(&root, 0, &targets, &mut found);
        for f in found {
            util::recycle(&f, "删除快捷方式", report);
        }
    }

    log("残留扫描完成");
}

fn util_roots() -> Vec<(HKEY, &'static str, &'static str)> {
    crate::apps::uninstall_roots()
}

type Target = (String, String);

fn shortcut_matches(app_name: &str) -> Vec<Target> {
    // (归一化名, 原名) —— 匹配用归一化，展示用原名
    vec![(util::normalize(app_name), app_name.to_string())]
}

/// 递归收集匹配的 .lnk 文件
fn walk_lnks(dir: &std::path::Path, depth: usize, targets: &[Target], out: &mut Vec<std::path::PathBuf>) {
    if depth > 6 {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for entry in rd.flatten() {
        let p = entry.path();
        if p.is_dir() {
            walk_lnks(&p, depth + 1, targets, out);
            continue;
        }
        let Some(stem) = p.file_stem().map(|s| s.to_string_lossy().to_string()) else { continue };
        let ext = p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
        if ext != "lnk" {
            continue;
        }
        for (norm, _raw) in targets {
            let s = util::normalize(&stem);
            if s == *norm || (s.starts_with(norm.as_str()) && util::version_like(&s[norm.len()..])) {
                out.push(p.clone());
                break;
            }
        }
    }
}

/// 标准卸载等待轮询：卸载键消失 或 进程退出后宽限，最多 5 分钟
pub fn wait_for_uninstaller(rec: &crate::Record, hprocess: *mut std::ffi::c_void, log: &dyn Fn(&str)) {
    let (hive, _, _) = {
        // 自身键所在根
        match crate::apps::uninstall_roots().into_iter().find(|(_, n, _)| *n == rec.hive_name) {
            Some(t) => t,
            None => return,
        }
    };
    let start = Instant::now();
    let mut exited = false;
    let mut grace = 0u32;
    log("等待卸载程序完成（最长 5 分钟）…");
    loop {
        if !util::key_exists(hive, &rec.uninstall_root, &rec.key_name) {
            log("卸载注册表键已消失，卸载完成");
            break;
        }
        if !exited && !hprocess.is_null() {
            exited = crate::launch::wait_handle(hprocess, 2000);
            if exited {
                log("卸载程序进程已退出");
            }
        } else {
            std::thread::sleep(Duration::from_secs(2));
        }
        if exited {
            grace += 1;
            if grace > 5 {
                log("进程已退出但注册表键仍在，继续扫描残留");
                break;
            }
        }
        if start.elapsed() > Duration::from_secs(300) {
            log("等待超时（5 分钟），继续扫描残留");
            break;
        }
    }
}
