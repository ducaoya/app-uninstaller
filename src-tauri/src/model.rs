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

impl Report {
    pub fn finalize(&mut self) {
        self.ok = !self.steps.iter().any(|s| s.status == "error");
    }
}
