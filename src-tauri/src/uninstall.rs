use crate::model::Report;
use crate::sweep;

/// 标准卸载主流程：唤起系统卸载器 → 等待 → 残留扫描清理
pub fn run(rec: &crate::Record, log: &dyn Fn(&str)) -> Report {
    let mut report = Report::default();
    let app = &rec.app;

    log(&format!("== 卸载 {} ==", app.name));

    let us = app.uninstall_string.trim();
    if us.is_empty() {
        report.steps.push(crate::model::Step::error(
            "启动卸载程序",
            &app.name,
            "注册表无 UninstallString",
        ));
        report.finalize();
        return report;
    }

    // 启动卸载器
    let hprocess = if app.is_msi {
        match crate::launch::product_code(us) {
            Some(guid) => {
                log("唤起系统卸载程序（msiexec）…");
                match crate::launch::shellexecute("msiexec.exe", &format!("/x {guid}")) {
                    Ok(h) => Some(h),
                    Err(e) => {
                        report.steps.push(crate::model::Step::error("启动卸载程序", "msiexec", e));
                        None
                    }
                }
            }
            None => {
                report.steps.push(crate::model::Step::error(
                    "启动卸载程序",
                    us,
                    "MSI 条目未找到 ProductCode",
                ));
                None
            }
        }
    } else {
        let (exe, args) = crate::launch::split_cmd(us);
        log(&format!("唤起系统卸载程序:{exe}"));
        match crate::launch::shellexecute(&exe, &args) {
            Ok(h) => Some(h),
            Err(e) => {
                report.steps.push(crate::model::Step::error("启动卸载程序", &exe, e));
                None
            }
        }
    };

    // 等待完成
    if let Some(h) = hprocess {
        sweep::wait_for_uninstaller(rec, h, log);
        crate::launch::close_handle(h);
    } else {
        log("未能跟踪卸载进程，直接进入残留扫描");
    }

    // 残留扫描 + 清理
    sweep::run(rec, &mut report, log);

    report.finalize();
    report
}
