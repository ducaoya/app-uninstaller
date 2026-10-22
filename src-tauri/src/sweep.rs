use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::model::{log_step, Logger, Report, Step};
use crate::util::{self};

/// 取消时：记录日志 + 报告步骤
fn mark_cancelled(report: &mut Report, log: Logger) {
    log("warn", "任务已取消，剩余清理项跳过".to_string());
    let step = Step::skipped("清理已取消", "剩余项未处理", "");
    log_step(log, &step);
    report.steps.push(step);
}

/// 扫描并清理残留（标准卸载后 / 强制卸载末尾共用）
pub fn run(rec: &crate::Record, report: &mut Report, log: Logger, cancel: &dyn Fn() -> bool) {
    let app = &rec.app;
    log("info", "开始扫描残留…".to_string());

    // 1. 三个根下的同名卸载键
    for (hive, hive_name, root) in crate::apps::uninstall_roots() {
        if cancel() {
            mark_cancelled(report, log);
            return;
        }
        log("info", format!("扫描卸载键 {}\\{}", hive_name, root));
        util::delete_key_tree(hive, hive_name, root, &rec.key_name, "删除卸载键", report, log);
    }

    // 2. 厂商 / 应用名注册表键
    for (hive, hive_name, prefix, label) in
        util::publisher_key_candidates(&app.publisher, &app.name)
    {
        if cancel() {
            mark_cancelled(report, log);
            return;
        }
        log("info", format!("扫描厂商键 {}\\{}\\{}", hive_name, prefix, label));
        util::delete_key_tree(hive, hive_name, &prefix, &label, "删除厂商键", report, log);
    }

    // 3. 安装目录残余（标准卸载后可能仍在；带保护路径检查）
    let install = app.install_location.trim().to_string();
    if !install.is_empty() {
        if cancel() {
            mark_cancelled(report, log);
            return;
        }
        let dir = Path::new(&install);
        if dir.exists() && !util::is_protected(dir) {
            log("info", format!("扫描安装目录残余：{}", install));
            util::recycle(dir, "清理安装目录残余", report, log);
        }
    }

    // 4. AppData / ProgramData 同名目录（回收站）
    for dir in util::appdata_candidates(&app.name, &app.publisher) {
        if cancel() {
            mark_cancelled(report, log);
            return;
        }
        log("info", format!("扫描数据目录：{}", dir.display()));
        util::recycle(&dir, "清理数据目录", report, log);
    }

    // 5. 快捷方式（回收站）
    for root in util::shortcut_roots() {
        if cancel() {
            mark_cancelled(report, log);
            return;
        }
        log("info", format!("扫描快捷方式：{}", root.display()));
        let mut found = Vec::new();
        walk_lnks(&root, 0, &util::normalize(&app.name), &mut found);
        for f in found {
            util::recycle(&f, "删除快捷方式", report, log);
        }
    }

    log("ok", "残留扫描完成".to_string());
}

/// 递归收集匹配的 .lnk 文件
fn walk_lnks(dir: &std::path::Path, depth: usize, norm_name: &str, out: &mut Vec<PathBuf>) {
    if depth > 6 || norm_name.is_empty() {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for entry in rd.flatten() {
        let p = entry.path();
        if p.is_dir() {
            walk_lnks(&p, depth + 1, norm_name, out);
            continue;
        }
        let Some(stem) = p.file_stem().map(|s| s.to_string_lossy().to_string()) else { continue };
        let ext = p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
        if ext != "lnk" {
            continue;
        }
        let s = util::normalize(&stem);
        if s == *norm_name || (s.starts_with(norm_name) && util::version_like(&s[norm_name.len()..])) {
            out.push(p);
        }
    }
}

/// 标准卸载等待轮询：卸载键消失 或 进程退出后宽限，最多 5 分钟；可取消
pub fn wait_for_uninstaller(
    rec: &crate::Record,
    hprocess: *mut std::ffi::c_void,
    log: Logger,
    cancel: &dyn Fn() -> bool,
) {
    let Some((hive, _, _)) = crate::apps::uninstall_roots()
        .into_iter()
        .find(|(_, n, _)| *n == rec.hive_name)
    else {
        return;
    };
    let start = Instant::now();
    let mut exited = false;
    let mut grace = 0u32;
    log("info", "等待卸载程序完成（最长 5 分钟）…".to_string());
    loop {
        if cancel() {
            log("warn", "已取消等待卸载程序".to_string());
            return;
        }
        if !util::key_exists(hive, &rec.uninstall_root, &rec.key_name) {
            log("ok", "卸载注册表键已消失，卸载完成".to_string());
            break;
        }
        if !exited && !hprocess.is_null() {
            exited = crate::launch::wait_handle(hprocess, 2000);
            if exited {
                log("info", "卸载程序进程已退出".to_string());
            }
        } else {
            std::thread::sleep(Duration::from_secs(2));
        }
        if exited {
            grace += 1;
            if grace > 5 {
                log("warn", "进程已退出但注册表键仍在，继续扫描残留".to_string());
                break;
            }
        }
        if start.elapsed() > Duration::from_secs(300) {
            log("warn", "等待超时（5 分钟），继续扫描残留".to_string());
            break;
        }
    }
}
