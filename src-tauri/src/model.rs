use serde::{Deserialize, Serialize};

/// 单条应用信息（来自注册表卸载键）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    pub publisher: String,
    pub install_location: String,
    pub uninstall_string: String,
    pub size_kb: u64,
    pub install_date: String,
    pub is_msi: bool,
}

/// 一次操作的步骤记录
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Step {
    pub action: String,
    pub target: String,
    /// "done" | "not_found" | "error" | "skipped"
    pub status: String,
    pub detail: String,
}

impl Step {
    pub fn done(action: &str, target: &str) -> Self {
        Step { action: action.into(), target: target.into(), status: "done".into(), detail: String::new() }
    }
    pub fn not_found(action: &str, target: &str) -> Self {
        Step { action: action.into(), target: target.into(), status: "not_found".into(), detail: String::new() }
    }
    pub fn error(action: &str, target: &str, detail: impl Into<String>) -> Self {
        Step { action: action.into(), target: target.into(), status: "error".into(), detail: detail.into() }
    }
    pub fn skipped(action: &str, target: &str, detail: impl Into<String>) -> Self {
        Step { action: action.into(), target: target.into(), status: "skipped".into(), detail: detail.into() }
    }
}

/// 操作报告
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub ok: bool,
    pub steps: Vec<Step>,
}

/// 实时日志行（level: info=详情 | ok=正常 | warn=警告 | err=错误）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogLine {
    pub level: String,
    pub text: String,
}

/// 日志回调：(level, text)
pub type Logger<'a> = &'a dyn Fn(&str, String);

/// 把步骤结果作为核心日志发出
pub fn log_step(log: Logger, step: &Step) {
    let level = match step.status.as_str() {
        "done" => "ok",
        "error" => "err",
        _ => "warn",
    };
    let detail = if step.detail.is_empty() {
        String::new()
    } else {
        format!("（{}）", step.detail)
    };
    log(level, format!("{} · {}{}", step.action, step.target, detail));
}

impl Report {
    pub fn finalize(&mut self) {
        self.ok = !self.steps.iter().any(|s| s.status == "error");
    }
}
