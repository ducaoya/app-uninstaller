use crate::model::{log_step, Logger, Report, Step};
use crate::sweep;

/// 标准卸载主流程：唤起系统卸载器 → 等待 → 残留扫描清理
pub fn run(rec: &crate::Record, log: Logger, cancel: &dyn Fn() -> bool) -> Report {
    let mut report = Report::default();
    let app = &rec.app;

    log("info", format!("—— 卸载 {} ——", app.name));

    let us = app.uninstall_string.trim();
    if us.is_empty() {
        let step = Step::error("启动卸载程序", &app.name, "注册表无 UninstallString");
        log_step(log, &step);
        report.steps.push(step);
        report.finalize();
        return report;
    }

    // 启动卸载器
    let hprocess = if app.is_msi {
        match crate::launch::product_code(us) {
            Some(guid) => {
                log("info", "唤起系统卸载程序（msiexec）…".to_string());
                match crate::launch::shellexecute("msiexec.exe", &format!("/x {guid}")) {
                    Ok(h) => Some(h),
                    Err(e) => {
                        let step = Step::error("启动卸载程序", "msiexec", e);
                        log_step(log, &step);
                        report.steps.push(step);
                        None
                    }
                }
            }
            None => {
                let step = Step::error("启动卸载程序", us, "MSI 条目未找到 ProductCode");
                log_step(log, &step);
                report.steps.push(step);
                None
            }
        }
    } else {
        let (exe, args) = crate::launch::split_cmd(us);
        log("info", format!("唤起系统卸载程序：{exe}"));
        match crate::launch::shellexecute(&exe, &args) {
            Ok(h) => Some(h),
            Err(e) => {
                let step = Step::error("启动卸载程序", &exe, e);
                log_step(log, &step);
                report.steps.push(step);
                None
            }
        }
    };

    // 等待完成（可取消）
    if let Some(h) = hprocess {
        sweep::wait_for_uninstaller(rec, h, log, cancel);
        crate::launch::close_handle(h);
    } else {
        log("info", "未能跟踪卸载进程，直接进入残留扫描".to_string());
    }

    // 取消 → 跳过清理
    if cancel() {
        log("warn", "任务已取消，跳过残留清理".to_string());
        let step = Step::skipped("残留清理", "任务已取消", "");
        log_step(log, &step);
        report.steps.push(step);
        report.finalize();
        return report;
    }

    // 残留扫描 + 清理
    sweep::run(rec, &mut report, log, cancel);

    report.finalize();
    report
}
