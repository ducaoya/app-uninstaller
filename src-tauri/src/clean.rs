use crate::model::Report;
use crate::util;

/// 清除注册表：卸载键（三个根同名）+ 厂商 / 应用名键
pub fn registry(rec: &crate::Record, log: &dyn Fn(&str)) -> Report {
    let mut report = Report::default();
    let app = &rec.app;
    log(&format!("== 清除注册表 {} ==", app.name));

    for (hive, hive_name, root) in crate::apps::uninstall_roots() {
        util::delete_key_tree(hive, hive_name, root, &rec.key_name, "删除卸载键", &mut report);
    }
    for (hive, hive_name, prefix, label) in
        util::publisher_key_candidates(&app.publisher, &app.name)
    {
        util::delete_key_tree(hive, hive_name, &prefix, &label, "删除厂商键", &mut report);
    }

    report.finalize();
    report
}

/// 清除缓存文件夹：AppData 下同名目录（进回收站，不动安装目录）
pub fn cache(rec: &crate::Record, log: &dyn Fn(&str)) -> Report {
    let mut report = Report::default();
    let app = &rec.app;
    log(&format!("== 清除缓存 {} ==", app.name));

    let dirs = util::appdata_candidates(&app.name, &app.publisher);
    if dirs.is_empty() {
        log("未发现匹配的数据目录");
        report.steps.push(crate::model::Step::not_found("清理数据目录", "无匹配目录"));
    }
    for dir in dirs {
        util::recycle(&dir, "清理数据目录", &mut report);
    }

    report.finalize();
    report
}
