/** 与 Rust model.rs / apps.rs 对应 */
export interface AppInfo {
  id: string;
  name: string;
  version: string;
  publisher: string;
  installLocation: string;
  uninstallString: string;
  sizeKb: number;
  installDate: string;
  isMsi: boolean;
}

export interface Step {
  action: string;
  target: string;
  status: "done" | "not_found" | "error" | "skipped";
  detail: string;
}

export interface Report {
  ok: boolean;
  steps: Step[];
}

/** 实时日志行（level: info=详情 | ok=正常 | warn=警告 | err=错误） */
export interface LogLine {
  level: "info" | "ok" | "warn" | "err";
  text: string;
}
