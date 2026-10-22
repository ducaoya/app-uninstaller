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
