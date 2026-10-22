use std::path::Path;

use sysinfo::System;

use crate::model::{log_step, Logger, Report, Step};
use crate::util;

/// 结束运行于指定目录下的所有进程
pub fn kill_processes_under(dir: &str, log: Logger) {
    let dir_norm = dir.trim_end_matches(['\\', '/']).to_lowercase();
    if dir_norm.is_empty() {
        return;
    }
    log("info", format!("正在扫描运行中的进程（{}）…", dir));
    let sys = System::new_all();
    let mut killed = 0usize;
    for (pid, process) in sys.processes() {
        let Some(exe) = process.exe() else { continue };
        let exe_str = exe.to_string_lossy().to_lowercase();
        if exe_str.starts_with(&dir_norm) {
            let name = process.name().to_string_lossy().to_string();
            let step = match std::process::Command::new("taskkill")
                .args(["/PID", &pid.as_u32().to_string(), "/T", "/F"])
                .output()
            {
                Ok(o) if o.status.success() => {
                    killed += 1;
                    Step::done(
                        "结束进程",
                        &format!("{name}（PID {}）", pid.as_u32()),
                    )
                }
                _ => {
                    log("warn", format!("结束进程失败:{name}（PID {}）", pid.as_u32()));
                    continue;
                }
            };
            log_step(log, &step);
        }
    }
    if killed == 0 {
        log("info", "未发现该目录下运行中的进程".to_string());
    }
}

/// 强制卸载主流程
pub fn run(rec: &crate::Record, log: Logger, cancel: &dyn Fn() -> bool) -> Report {
    let mut report = Report::default();
    let app = &rec.app;

    log("info", format!("—— 强制卸载 {} ——", app.name));

    // 1. 结束进程
    if !app.install_location.trim().is_empty() {
        kill_processes_under(&app.install_location, log);
        // 2. 直接删除安装目录
        let dir = Path::new(&app.install_location);
        if cancel() {
            mark_cancelled(&mut report, log);
            return report;
        }
        if util::is_protected(dir) {
            let step = Step::skipped("删除安装目录", &app.install_location, "命中系统保护路径");
            log_step(log, &step);
            report.steps.push(step);
        } else {
            util::delete_dir_now(dir, "删除安装目录", &mut report, log);
        }
    } else {
        log("info", "无安装目录信息，跳过文件删除".to_string());
        let step = Step::skipped("删除安装目录", "未知", "注册表无 InstallLocation");
        log_step(log, &step);
        report.steps.push(step);
    }

    // 3. MSI 静默卸载，避免孤儿记录
    if app.is_msi {
        if let Some(guid) = crate::launch::product_code(&app.uninstall_string) {
            if cancel() {
                mark_cancelled(&mut report, log);
                return report;
            }
            log("info", "执行 msiexec /x 静默卸载…".to_string());
            let step = match crate::launch::msiexec_quiet(&guid) {
                Ok(()) => Step::done("MSI 静默卸载", &guid),
                Err(e) => Step::error("MSI 静默卸载", &guid, e),
            };
            log_step(log, &step);
            report.steps.push(step);
        }
    }

    // 4. 注册表 + 残留扫描
    if cancel() {
        mark_cancelled(&mut report, log);
        return report;
    }
    crate::sweep::run(rec, &mut report, log, cancel);

    report.finalize();
    report
}

fn mark_cancelled(report: &mut Report, log: Logger) {
    log("warn", "任务已取消，剩余步骤跳过".to_string());
    let step = Step::skipped("任务已取消", "剩余步骤未执行", "");
    log_step(log, &step);
    report.steps.push(step);
}
