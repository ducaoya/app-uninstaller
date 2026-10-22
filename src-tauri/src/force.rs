use std::path::Path;

use sysinfo::System;

use crate::model::{Report, Step};
use crate::util::{self};

/// 结束运行于指定目录下的所有进程
pub fn kill_processes_under(dir: &str, log: &dyn Fn(&str)) {
    let dir_norm = dir.trim_end_matches(['\\', '/']).to_lowercase();
    if dir_norm.is_empty() {
        return;
    }
    let sys = System::new_all();
    let mut killed = 0usize;
    for (pid, process) in sys.processes() {
        let Some(exe) = process.exe() else { continue };
        let exe_str = exe.to_string_lossy().to_lowercase();
        if exe_str.starts_with(&dir_norm) {
            let name = process.name().to_string_lossy().to_string();
            let taskkill = std::process::Command::new("taskkill")
                .args(["/PID", &pid.as_u32().to_string(), "/T", "/F"])
                .output();
            match taskkill {
                Ok(o) if o.status.success() => {
                    killed += 1;
                    log(&format!("已结束进程 {name}（PID {}）", pid.as_u32()));
                }
                _ => log(&format!("结束进程失败:{name}（PID {}）", pid.as_u32())),
            }
        }
    }
    if killed == 0 {
        log("未发现该目录下运行中的进程");
    }
}

/// 强制卸载主流程
pub fn run(rec: &crate::Record, log: &dyn Fn(&str)) -> Report {
    let mut report = Report::default();
    let app = &rec.app;

    log(&format!("== 强制卸载 {} ==", app.name));

    // 1. 结束进程
    if !app.install_location.trim().is_empty() {
        kill_processes_under(&app.install_location, log);
        // 2. 直接删除安装目录
        let dir = Path::new(&app.install_location);
        if util::is_protected(dir) {
            report.steps.push(Step::skipped(
                "删除安装目录",
                &app.install_location,
                "命中系统保护路径",
            ));
        } else {
            util::delete_dir_now(dir, "删除安装目录", &mut report);
        }
    } else {
        log("无安装目录信息，跳过文件删除");
        report.steps.push(Step::skipped("删除安装目录", "未知", "注册表无 InstallLocation"));
    }

    // 3. MSI 静默卸载，避免孤儿记录
    if app.is_msi {
        if let Some(guid) = crate::launch::product_code(&app.uninstall_string) {
            log("执行 msiexec /x 静默卸载…");
            match crate::launch::msiexec_quiet(&guid) {
                Ok(()) => report.steps.push(Step::done("MSI 静默卸载", &guid)),
                Err(e) => report.steps.push(Step::error("MSI 静默卸载", &guid, e)),
            }
        }
    }

    // 4. 注册表 + 残留扫描
    crate::sweep::run(rec, &mut report, log);

    report.finalize();
    report
}
