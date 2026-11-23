// windows specific parts: windows hello, notifications, display and title bar

// returns true when there is nothing to lock with, so the user is never locked out
pub fn verify(hwnd: isize, message: &str) -> bool {
    #[cfg(windows)]
    {
        use windows::Security::Credentials::UI::{
            UserConsentVerificationResult, UserConsentVerifier, UserConsentVerifierAvailability,
        };
        use windows::Win32::Foundation::HWND;
        use windows::Win32::System::WinRT::IUserConsentVerifierInterop;
        use windows::core::{HSTRING, factory};

        let available = UserConsentVerifier::CheckAvailabilityAsync()
            .and_then(|op| op.join())
            .map(|a| a == UserConsentVerifierAvailability::Available)
            .unwrap_or(false);
        if !available {
            return true;
        }

        let result = (|| -> windows::core::Result<UserConsentVerificationResult> {
            let interop = factory::<UserConsentVerifier, IUserConsentVerifierInterop>()?;
            let operation: windows_future::IAsyncOperation<UserConsentVerificationResult> = unsafe {
                interop.RequestVerificationForWindowAsync(HWND(hwnd as *mut core::ffi::c_void), &HSTRING::from(message))?
            };
            operation.join()
        })();
        matches!(result, Ok(UserConsentVerificationResult::Verified))
    }
    #[cfg(not(windows))]
    {
        let _ = (hwnd, message);
        true
    }
}

// app user model id, the installer gives the start menu shortcut the same id
pub const APP_ID: &str = "Kairo.Yomikaze";

pub fn register_app_id() {
    #[cfg(windows)]
    unsafe {
        use windows::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID;
        use windows::core::HSTRING;
        let _ = SetCurrentProcessExplicitAppUserModelID(&HSTRING::from(APP_ID));
    }
}

pub fn keep_display_on(on: bool) {
    #[cfg(windows)]
    {
        use std::sync::atomic::{AtomicBool, Ordering};
        use windows::Win32::System::Power::{ES_CONTINUOUS, ES_DISPLAY_REQUIRED, SetThreadExecutionState};

        static CURRENT: AtomicBool = AtomicBool::new(false);
        if CURRENT.swap(on, Ordering::Relaxed) == on {
            return;
        }
        unsafe {
            if on {
                SetThreadExecutionState(ES_CONTINUOUS | ES_DISPLAY_REQUIRED);
            } else {
                SetThreadExecutionState(ES_CONTINUOUS);
            }
        }
    }
    #[cfg(not(windows))]
    let _ = on;
}

// windows 11 only, older versions keep the normal title bar
pub fn style_title_bar(hwnd: isize, dark: bool, caption: [u8; 3], text: [u8; 3]) {
    #[cfg(windows)]
    {
        use windows::Win32::Foundation::{COLORREF, HWND};
        use windows::Win32::Graphics::Dwm::{
            DWMWA_BORDER_COLOR, DWMWA_CAPTION_COLOR, DWMWA_TEXT_COLOR, DWMWA_USE_IMMERSIVE_DARK_MODE, DwmSetWindowAttribute,
        };
        if hwnd == 0 {
            return;
        }
        let hwnd = HWND(hwnd as *mut core::ffi::c_void);
        let colorref = |c: [u8; 3]| COLORREF(c[0] as u32 | (c[1] as u32) << 8 | (c[2] as u32) << 16);
        let dark_mode: i32 = dark as i32;
        let caption = colorref(caption);
        let text = colorref(text);
        let size = std::mem::size_of::<COLORREF>() as u32;
        unsafe {
            let _ = DwmSetWindowAttribute(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE, &dark_mode as *const i32 as *const _, 4);
            let _ = DwmSetWindowAttribute(hwnd, DWMWA_CAPTION_COLOR, &caption as *const COLORREF as *const _, size);
            let _ = DwmSetWindowAttribute(hwnd, DWMWA_BORDER_COLOR, &caption as *const COLORREF as *const _, size);
            let _ = DwmSetWindowAttribute(hwnd, DWMWA_TEXT_COLOR, &text as *const COLORREF as *const _, size);
        }
    }
    #[cfg(not(windows))]
    {
        let _ = (hwnd, dark, caption, text);
    }
}

// dpapi: only this windows user can read the data back. used for the anilist token.
pub fn protect(data: &[u8]) -> Vec<u8> {
    #[cfg(windows)]
    unsafe {
        use windows::Win32::Foundation::{HLOCAL, LocalFree};
        use windows::Win32::Security::Cryptography::{CRYPT_INTEGER_BLOB, CryptProtectData};
        let input = CRYPT_INTEGER_BLOB { cbData: data.len() as u32, pbData: data.as_ptr() as *mut u8 };
        let mut output = CRYPT_INTEGER_BLOB::default();
        if CryptProtectData(&input, None, None, None, None, 0, &mut output).is_ok() {
            let bytes = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
            let _ = LocalFree(Some(HLOCAL(output.pbData as *mut core::ffi::c_void)));
            return bytes;
        }
        Vec::new()
    }
    #[cfg(not(windows))]
    data.to_vec()
}

pub fn unprotect(data: &[u8]) -> Option<Vec<u8>> {
    #[cfg(windows)]
    unsafe {
        use windows::Win32::Foundation::{HLOCAL, LocalFree};
        use windows::Win32::Security::Cryptography::{CRYPT_INTEGER_BLOB, CryptUnprotectData};
        let input = CRYPT_INTEGER_BLOB { cbData: data.len() as u32, pbData: data.as_ptr() as *mut u8 };
        let mut output = CRYPT_INTEGER_BLOB::default();
        CryptUnprotectData(&input, None, None, None, None, 0, &mut output).ok()?;
        let bytes = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
        let _ = LocalFree(Some(HLOCAL(output.pbData as *mut core::ffi::c_void)));
        Some(bytes)
    }
    #[cfg(not(windows))]
    Some(data.to_vec())
}

// true when another yomikaze is already running. the mutex lives as long as this process.
pub fn another_instance_running() -> bool {
    #[cfg(windows)]
    unsafe {
        use windows::Win32::Foundation::{ERROR_ALREADY_EXISTS, GetLastError};
        use windows::Win32::System::Threading::CreateMutexW;
        use windows::core::w;
        let handle = CreateMutexW(None, false, w!("Yomikaze.Running"));
        let exists = GetLastError() == ERROR_ALREADY_EXISTS;
        std::mem::forget(handle);
        exists
    }
    #[cfg(not(windows))]
    false
}

// makes yomikaze:// links open this exe (per user, no admin rights needed)
pub fn register_link_protocol() {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const NO_WINDOW: u32 = 0x0800_0000;
        let Ok(exe) = std::env::current_exe() else { return };
        let command = format!("\"{}\" \"%1\"", exe.display());
        let key = r"HKCU\Software\Classes\yomikaze";
        let runs: [&[&str]; 3] = [
            &["add", key, "/ve", "/d", "URL:Yomikaze", "/f"],
            &["add", key, "/v", "URL Protocol", "/d", "", "/f"],
            &["add", &format!(r"{key}\shell\open\command"), "/ve", "/d", &command, "/f"],
        ];
        for args in runs {
            let _ = std::process::Command::new("reg").args(args).creation_flags(NO_WINDOW).status();
        }
    }
}
