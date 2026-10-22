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
    /// 本步耗时（毫秒，0 = 未计时）
    #[serde(default)]
    pub elapsed_ms: u64,
}

impl Step {
    fn new(action: &str, target: &str, status: &str, detail: impl Into<String>) -> Self {
        Step {
            action: action.into(),
            target: target.into(),
            status: status.into(),
            detail: detail.into(),
            elapsed_ms: 0,
        }
    }
    pub fn done(action: &str, target: &str) -> Self {
        Step::new(action, target, "done", "")
    }
    pub fn not_found(action: &str, target: &str) -> Self {
        Step::new(action, target, "not_found", "")
    }
    pub fn error(action: &str, target: &str, detail: impl Into<String>) -> Self {
        Step::new(action, target, "error", detail)
    }
    pub fn skipped(action: &str, target: &str, detail: impl Into<String>) -> Self {
        Step::new(action, target, "skipped", detail)
    }
    /// 附上实测耗时
    pub fn timed(mut self, start: std::time::Instant) -> Self {
        self.elapsed_ms = start.elapsed().as_millis() as u64;
        self
    }
    /// 人读耗时（如 3.2s / 1m02s），小于 100ms 返回空
    pub fn duration_text(&self) -> String {
        fmt_duration(self.elapsed_ms)
    }
}

pub fn fmt_duration(ms: u64) -> String {
    if ms < 100 {
        return String::new();
    }
    let secs = ms as f64 / 1000.0;
    if secs < 60.0 {
        format!("{:.1}s", secs)
    } else {
        format!("{}m{:02}s", secs as u64 / 60, (secs as u64) % 60)
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

/// 把步骤结果作为核心日志发出（含耗时）
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
    let dur = step.duration_text();
    let dur = if dur.is_empty() { String::new() } else { format!(" [{}]", dur) };
    log(level, format!("{} · {}{}{}", step.action, step.target, detail, dur));
}

impl Report {
    pub fn finalize(&mut self) {
        self.ok = !self.steps.iter().any(|s| s.status == "error");
    }
}
