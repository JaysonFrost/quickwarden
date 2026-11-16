#[derive(Clone, Debug, Default)]
pub struct Ctx {
    pub handle: isize,
    pub title: String,
    pub app: String,
    pub url: String,
}

#[cfg(windows)]
pub use win::{active, focus, url};

#[cfg(target_os = "macos")]
pub use mac::{active, focus, url};

#[cfg(windows)]
mod win {
    use super::Ctx;
    use std::ptr::null_mut;
    use windows_sys::Win32::Foundation::{CloseHandle, HWND};
    use windows_sys::Win32::System::Threading::{
        AttachThreadInput, GetCurrentThreadId, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
        QueryFullProcessImageNameW,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId,
        SetForegroundWindow,
    };

    const BROWSERS: [&str; 7] = [
        "chrome", "msedge", "brave", "vivaldi", "opera", "firefox", "arc",
    ];

    fn process_name(hwnd: HWND) -> String {
        let mut pid = 0u32;
        unsafe {
            GetWindowThreadProcessId(hwnd, &mut pid);
            let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if handle.is_null() {
                return String::new();
            }
            let mut buf = [0u16; 1024];
            let mut len = buf.len() as u32;
            let ok = QueryFullProcessImageNameW(handle, 0, buf.as_mut_ptr(), &mut len);
            CloseHandle(handle);
            if ok == 0 {
                return String::new();
            }
            let path = String::from_utf16_lossy(&buf[..len as usize]);
            let file = path.rsplit('\\').next().unwrap_or("");
            file.strip_suffix(".exe")
                .or(file.strip_suffix(".EXE"))
                .unwrap_or(file)
                .to_string()
        }
    }

    pub fn active() -> Ctx {
        unsafe {
            let hwnd = GetForegroundWindow();
            let len = GetWindowTextLengthW(hwnd);
            let mut buf = vec![0u16; len as usize + 1];
            let n = GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32);
            Ctx {
                handle: hwnd as isize,
                title: String::from_utf16_lossy(&buf[..n.max(0) as usize]),
                app: process_name(hwnd),
                url: String::new(),
            }
        }
    }

    pub fn focus(ctx: &Ctx) {
        if ctx.handle == 0 {
            return;
        }
        unsafe {
            let theirs = GetWindowThreadProcessId(GetForegroundWindow(), null_mut());
            let mine = GetCurrentThreadId();
            AttachThreadInput(mine, theirs, 1);
            SetForegroundWindow(ctx.handle as HWND);
            AttachThreadInput(mine, theirs, 0);
        }
    }

    pub fn url(ctx: &Ctx) -> String {
        use uiautomation::UIAutomation;
        use uiautomation::patterns::UIValuePattern;
        use uiautomation::types::{ControlType, Handle};

        if !BROWSERS.contains(&ctx.app.to_lowercase().as_str()) {
            return String::new();
        }
        let find = || -> uiautomation::Result<String> {
            let ui = UIAutomation::new()?;
            let window = ui.element_from_handle(Handle::from(ctx.handle))?;
            let edit = ui
                .create_matcher()
                .from(window)
                .control_type(ControlType::Edit)
                .depth(20)
                .timeout(0)
                .find_first()?;
            edit.get_pattern::<UIValuePattern>()?.get_value()
        };
        find().unwrap_or_default()
    }

    #[test]
    #[ignore]
    fn reads_browser_url_from_env_window() {
        let handle: isize = std::env::var("QUICKWARDEN_TEST_HWND")
            .unwrap()
            .parse()
            .unwrap();
        let ctx = Ctx {
            handle,
            app: std::env::var("QUICKWARDEN_TEST_APP").unwrap(),
            ..Default::default()
        };
        let url = url(&ctx);
        println!("url={url}");
        assert!(!url.is_empty());
    }
}

#[cfg(target_os = "macos")]
mod mac {
    use super::Ctx;
    use std::process::Command;

    const CHROMIUM: [&str; 7] = [
        "Google Chrome",
        "Arc",
        "Brave Browser",
        "Microsoft Edge",
        "Vivaldi",
        "Chromium",
        "Opera",
    ];

    fn osa(script: &str) -> String {
        Command::new("osascript")
            .args(["-e", script])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_default()
    }

    fn quote(s: &str) -> String {
        s.replace('\\', "\\\\").replace('"', "\\\"")
    }

    pub fn active() -> Ctx {
        let out = osa(concat!(
            "tell application \"System Events\"\n",
            "set p to first application process whose frontmost is true\n",
            "set t to \"\"\n",
            "try\nset t to name of front window of p\nend try\n",
            "return (name of p) & linefeed & t\n",
            "end tell"
        ));
        let (app, title) = out.split_once('\n').unwrap_or((&out, ""));
        Ctx {
            handle: 0,
            title: title.to_string(),
            app: app.to_string(),
            url: String::new(),
        }
    }

    pub fn focus(ctx: &Ctx) {
        if !ctx.app.is_empty() {
            osa(&format!(
                "tell application \"System Events\" to set frontmost of process \"{}\" to true",
                quote(&ctx.app)
            ));
        }
    }

    pub fn url(ctx: &Ctx) -> String {
        if ctx.app == "Safari" {
            osa("tell application \"Safari\" to get URL of front document")
        } else if CHROMIUM.contains(&ctx.app.as_str()) {
            osa(&format!(
                "tell application \"{}\" to get URL of active tab of front window",
                quote(&ctx.app)
            ))
        } else {
            String::new()
        }
    }
}
