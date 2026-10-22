use std::ffi::c_void;
use std::os::windows::ffi::OsStrExt;
use std::ffi::OsStr;

use windows_sys::Win32::Foundation::{CloseHandle, WAIT_OBJECT_0};
use windows_sys::Win32::System::Threading::WaitForSingleObject;
use windows_sys::Win32::UI::Shell::{ShellExecuteExW, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW};
use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

/// UTF-16 宽字符串（NUL 结尾）
pub fn wide(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
}

/// 拆解 UninstallString 为 (exe, args)
pub fn split_cmd(s: &str) -> (String, String) {
    let s = s.trim();
    if let Some(rest) = s.strip_prefix('"') {
        if let Some(end) = rest.find('"') {
            return (rest[..end].to_string(), rest[end + 1..].trim().to_string());
        }
    }
    let lower = s.to_lowercase();
    if let Some(pos) = lower.find(".exe") {
        let cut = pos + 4;
        return (s[..cut].to_string(), s[cut..].trim().to_string());
    }
    (s.to_string(), String::new())
}

/// 从 UninstallString 提取 MSI ProductCode（{GUID}）
pub fn product_code(s: &str) -> Option<String> {
    let start = s.find('{')?;
    let end = s[start..].find('}')? + start;
    Some(s[start..=end].to_string())
}

/// ShellExecute 启动，返回进程句柄（可能为 NULL，如 DDE 情形）
pub fn shellexecute(file: &str, args: &str) -> Result<*mut c_void, String> {
    let file_w = wide(file);
    let args_w = wide(args);
    let mut sei: SHELLEXECUTEINFOW = unsafe { std::mem::zeroed() };
    sei.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
    sei.fMask = SEE_MASK_NOCLOSEPROCESS;
    sei.lpFile = file_w.as_ptr();
    sei.lpParameters = if args.is_empty() { std::ptr::null() } else { args_w.as_ptr() };
    sei.nShow = SW_SHOWNORMAL as i32;

    let ok = unsafe { ShellExecuteExW(&mut sei) };
    if ok == 0 {
        return Err(format!("ShellExecute 启动失败: {file} {args}"));
    }
    Ok(sei.hProcess)
}

/// 等待句柄退出（一次等待最多 timeout_ms 毫秒），返回是否已退出
pub fn wait_handle(h: *mut c_void, timeout_ms: u32) -> bool {
    if h.is_null() {
        return false;
    }
    unsafe { WaitForSingleObject(h, timeout_ms) == WAIT_OBJECT_0 }
}


/// 关闭句柄
pub fn close_handle(h: *mut c_void) {
    if !h.is_null() {
        unsafe { CloseHandle(h) };
    }
}

/// 静默 MSI 卸载（等待完成）
pub fn msiexec_quiet(guid: &str) -> Result<(), String> {
    let out = std::process::Command::new("msiexec.exe")
        .args(["/x", guid, "/qn", "/norestart"])
        .output()
        .map_err(|e| format!("msiexec 启动失败: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        let code = out.status.code().unwrap_or(-1);
        Err(format!("msiexec 退出码 {code}（0=成功 1605=未安装 3010=需重启）"))
    }
}
