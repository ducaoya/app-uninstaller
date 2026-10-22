use crate::model::{log_step, Logger, Report, Step};
use crate::util;

fn mark_cancelled(report: &mut Report, log: Logger) {
    log("warn", "任务已取消，剩余项跳过".to_string());
    let step = Step::skipped("清理已取消", "剩余项未处理", "");
    log_step(log, &step);
    report.steps.push(step);
}

/// 清除注册表：卸载键（三个根同名）+ 厂商 / 应用名键
pub fn registry(rec: &crate::Record, log: Logger, cancel: &dyn Fn() -> bool) -> Report {
    let mut report = Report::default();
    let app = &rec.app;
    log("info", format!("—— 清除注册表 {} ——", app.name));

    for (hive, hive_name, root) in crate::apps::uninstall_roots() {
        if cancel() {
            mark_cancelled(&mut report, log);
            report.finalize();
            return report;
        }
        log("info", format!("扫描卸载键 {}\\{}", hive_name, root));
        util::delete_key_tree(hive, hive_name, root, &rec.key_name, "删除卸载键", &mut report, log);
    }
    for (hive, hive_name, prefix, label) in
        util::publisher_key_candidates(&app.publisher, &app.name)
    {
        if cancel() {
            mark_cancelled(&mut report, log);
            report.finalize();
            return report;
        }
        log("info", format!("扫描厂商键 {}\\{}\\{}", hive_name, prefix, label));
        util::delete_key_tree(hive, hive_name, &prefix, &label, "删除厂商键", &mut report, log);
    }

    log("ok", "注册表清理结束".to_string());
    report.finalize();
    report
}

/// 清除缓存文件夹：AppData 下同名目录（进回收站，不动安装目录）
pub fn cache(rec: &crate::Record, log: Logger, cancel: &dyn Fn() -> bool) -> Report {
    let mut report = Report::default();
    let app = &rec.app;
    log("info", format!("—— 清除缓存 {} ——", app.name));

    let dirs = util::appdata_candidates(&app.name, &app.publisher);
    if dirs.is_empty() {
        log("info", "未发现匹配的数据目录".to_string());
        let step = Step::not_found("清理数据目录", "无匹配目录");
        log_step(log, &step);
        report.steps.push(step);
    }
    for dir in dirs {
        if cancel() {
            mark_cancelled(&mut report, log);
            report.finalize();
            return report;
        }
        log("info", format!("扫描数据目录：{}", dir.display()));
        util::recycle(&dir, "清理数据目录", &mut report, log);
    }

    log("ok", "缓存清理结束".to_string());
    report.finalize();
    report
}
