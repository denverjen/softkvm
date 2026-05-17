#[cfg(target_os = "windows")]
pub fn get_clipboard() -> Option<Vec<u8>> {
    use windows::Win32::Foundation::*;
    use windows::Win32::System::DataExchange::*;

    unsafe {
        if OpenClipboard(HWND(std::ptr::null_mut())).is_err() {
            return None;
        }
        let handle = GetClipboardData(CF_UNICODETEXT.0).ok();
        let result = handle.and_then(|h| {
            let ptr = GlobalLock(h) as *const u16;
            if ptr.is_null() {
                return None;
            }
            let len = (0..).take_while(|&i| *ptr.add(i) != 0).count();
            let slice = std::slice::from_raw_parts(ptr, len);
            let s = String::from_utf16_lossy(slice);
            GlobalUnlock(h).ok();
            Some(s.into_bytes())
        });
        let _ = CloseClipboard();
        result
    }
}

#[cfg(target_os = "windows")]
pub fn set_clipboard(data: &[u8]) -> anyhow::Result<()> {
    use windows::Win32::Foundation::*;
    use windows::Win32::System::DataExchange::*;
    use windows::Win32::GlobalMemory::*;

    unsafe {
        OpenClipboard(HWND(std::ptr::null_mut()))?;
        EmptyClipboard()?;

        let text = String::from_utf8_lossy(data);
        let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        let buf_len = wide.len() * 2;

        let h = GlobalAlloc(GMEM_MOVEABLE, buf_len)?;
        let ptr = GlobalLock(h) as *mut u16;
        std::ptr::copy_nonoverlapping(wide.as_ptr(), ptr, wide.len());
        GlobalUnlock(h)?;
        SetClipboardData(CF_UNICODETEXT.0, Some(h as _))?;
        CloseClipboard()?;
    }
    Ok(())
}

#[cfg(target_os = "linux")]
pub fn get_clipboard() -> Option<Vec<u8>> {
    std::process::Command::new("xclip")
        .args(["-selection", "clipboard", "-o"])
        .output()
        .ok()
        .map(|o| o.stdout)
}

#[cfg(target_os = "linux")]
pub fn set_clipboard(data: &[u8]) -> anyhow::Result<()> {
    use std::io::Write;
    let mut child = std::process::Command::new("xclip")
        .args(["-selection", "clipboard", "-i"])
        .stdin(std::process::Stdio::piped())
        .spawn()?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(data)?;
    }
    child.wait()?;
    Ok(())
}
