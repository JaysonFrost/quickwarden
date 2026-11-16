use std::time::Duration;

use enigo::{Direction, Enigo, Key, Keyboard, Settings};

use crate::totp;

const CLEAR_CLIPBOARD_AFTER: Duration = Duration::from_secs(30);

pub enum Part {
    Text(String),
    Tab,
    Totp(String),
}

pub fn type_parts(parts: &[Part]) -> Result<(), String> {
    let mut keys = Enigo::new(&Settings::default()).map_err(|e| e.to_string())?;
    let held = if cfg!(target_os = "macos") {
        &[Key::Shift, Key::Control, Key::Meta][..]
    } else {
        &[Key::Shift, Key::Control][..]
    };
    for &k in held {
        let _ = keys.key(k, Direction::Release);
    }
    for part in parts {
        match part {
            Part::Text(text) => keys.text(text),
            Part::Tab => keys.key(Key::Tab, Direction::Click),
            Part::Totp(secret) => {
                keys.text(&totp::current(secret).map(|c| c.code).unwrap_or_default())
            }
        }
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn set_clipboard(value: &str) -> Result<(), arboard::Error> {
    let mut clipboard = arboard::Clipboard::new()?;
    let set = clipboard.set();
    #[cfg(windows)]
    let set = {
        use arboard::SetExtWindows;
        set.exclude_from_history().exclude_from_cloud()
    };
    #[cfg(target_os = "macos")]
    let set = {
        use arboard::SetExtApple;
        set.exclude_from_history()
    };
    set.text(value)
}

pub fn copy_secret(value: String) -> Result<(), String> {
    set_clipboard(&value).map_err(|e| e.to_string())?;
    std::thread::spawn(move || {
        std::thread::sleep(CLEAR_CLIPBOARD_AFTER);
        if let Ok(mut clipboard) = arboard::Clipboard::new()
            && clipboard.get_text().is_ok_and(|current| current == value)
        {
            let _ = clipboard.set_text("");
        }
    });
    Ok(())
}
