pub const SUPPORTED: bool = cfg!(windows);

#[cfg(windows)]
pub use win::{clear, load, save};

#[cfg(not(windows))]
pub fn save(_session: &str) {}

#[cfg(not(windows))]
pub fn load() -> Option<String> {
    None
}

#[cfg(not(windows))]
pub fn clear() {}

#[cfg(windows)]
mod win {
    use windows::Win32::Foundation::{HLOCAL, LocalFree};
    use windows::Win32::Security::Cryptography::{
        CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData, CryptUnprotectData,
    };
    use windows::Win32::System::Registry::{
        HKEY, HKEY_CURRENT_USER, KEY_SET_VALUE, REG_BINARY, REG_OPTION_VOLATILE, RRF_RT_REG_BINARY,
        RegCloseKey, RegCreateKeyExW, RegDeleteTreeW, RegGetValueW, RegSetValueExW,
    };
    use windows::core::w;

    fn blob(data: &[u8]) -> CRYPT_INTEGER_BLOB {
        CRYPT_INTEGER_BLOB {
            cbData: data.len() as u32,
            pbData: data.as_ptr() as *mut u8,
        }
    }

    fn take(out: CRYPT_INTEGER_BLOB) -> Vec<u8> {
        unsafe {
            let bytes = std::slice::from_raw_parts(out.pbData, out.cbData as usize).to_vec();
            LocalFree(Some(HLOCAL(out.pbData.cast())));
            bytes
        }
    }

    pub fn save(session: &str) {
        let mut out = CRYPT_INTEGER_BLOB::default();
        let protected = unsafe {
            CryptProtectData(
                &blob(session.as_bytes()),
                w!("quickwarden"),
                None,
                None,
                None,
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut out,
            )
        };
        if protected.is_err() {
            return;
        }
        let data = take(out);
        let mut key = HKEY::default();
        unsafe {
            let created = RegCreateKeyExW(
                HKEY_CURRENT_USER,
                w!("Software\\Quickwarden"),
                None,
                None,
                REG_OPTION_VOLATILE,
                KEY_SET_VALUE,
                None,
                &mut key,
                None,
            );
            if created.is_ok() {
                let _ = RegSetValueExW(key, w!("session"), None, REG_BINARY, Some(&data));
                let _ = RegCloseKey(key);
            }
        }
    }

    pub fn load() -> Option<String> {
        let mut size = 0u32;
        unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                w!("Software\\Quickwarden"),
                w!("session"),
                RRF_RT_REG_BINARY,
                None,
                None,
                Some(&mut size),
            )
            .ok()
            .ok()?;
            let mut data = vec![0u8; size as usize];
            RegGetValueW(
                HKEY_CURRENT_USER,
                w!("Software\\Quickwarden"),
                w!("session"),
                RRF_RT_REG_BINARY,
                None,
                Some(data.as_mut_ptr().cast()),
                Some(&mut size),
            )
            .ok()
            .ok()?;
            data.truncate(size as usize);
            let mut out = CRYPT_INTEGER_BLOB::default();
            CryptUnprotectData(
                &blob(&data),
                None,
                None,
                None,
                None,
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut out,
            )
            .ok()?;
            String::from_utf8(take(out)).ok()
        }
    }

    pub fn clear() {
        unsafe {
            let _ = RegDeleteTreeW(HKEY_CURRENT_USER, w!("Software\\Quickwarden"));
        }
    }

    #[test]
    #[ignore]
    fn roundtrip_in_volatile_registry() {
        save("test-session");
        assert_eq!(load().as_deref(), Some("test-session"));
        clear();
        assert_eq!(load(), None);
    }
}
