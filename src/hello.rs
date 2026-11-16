#[cfg(windows)]
pub use win::{available, enroll, enrolled, forget, unlock};

#[cfg(not(windows))]
pub fn available() -> bool {
    false
}

#[cfg(not(windows))]
pub fn enrolled() -> bool {
    false
}

#[cfg(not(windows))]
pub fn enroll(_secret: &str) -> Result<(), String> {
    Err("Windows Hello is not available".into())
}

#[cfg(not(windows))]
pub fn unlock() -> Result<String, String> {
    Err("Windows Hello is not available".into())
}

#[cfg(not(windows))]
pub fn forget() {}

#[cfg(any(windows, test))]
mod sealed {
    use aes_gcm::aead::{Aead, KeyInit};
    use aes_gcm::{Aes256Gcm, Nonce};
    use sha2::{Digest, Sha256};

    pub fn seal(signature: &[u8], secret: &str) -> Result<Vec<u8>, String> {
        let cipher =
            Aes256Gcm::new_from_slice(&Sha256::digest(signature)).map_err(|e| e.to_string())?;
        let mut nonce = [0u8; 12];
        getrandom::fill(&mut nonce).map_err(|e| e.to_string())?;
        let mut out = nonce.to_vec();
        out.extend(
            cipher
                .encrypt(&Nonce::from(nonce), secret.as_bytes())
                .map_err(|_| "encryption failed")?,
        );
        Ok(out)
    }

    pub fn open(signature: &[u8], sealed: &[u8]) -> Result<String, String> {
        if sealed.len() < 12 + 16 {
            return Err("stored Windows Hello data is corrupt".into());
        }
        let cipher =
            Aes256Gcm::new_from_slice(&Sha256::digest(signature)).map_err(|e| e.to_string())?;
        let nonce: [u8; 12] = sealed[..12].try_into().map_err(|_| "bad nonce")?;
        let plain = cipher
            .decrypt(&Nonce::from(nonce), &sealed[12..])
            .map_err(|_| "Windows Hello key does not match stored data")?;
        String::from_utf8(plain).map_err(|e| e.to_string())
    }
}

#[cfg(windows)]
mod win {
    use std::path::PathBuf;
    use std::time::{Duration, Instant};

    use windows::Security::Credentials::{
        KeyCredential, KeyCredentialCreationOption, KeyCredentialManager, KeyCredentialStatus,
    };
    use windows::Security::Cryptography::CryptographicBuffer;
    use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx};
    use windows::Win32::UI::WindowsAndMessaging::{FindWindowW, SetForegroundWindow};
    use windows::core::{Array, HSTRING, w};

    use super::sealed;

    const NAME: &str = "bwquick";
    const CHALLENGE: &[u8] = b"bwquick-windows-hello-unlock-v1";

    fn file() -> Option<PathBuf> {
        Some(
            PathBuf::from(std::env::var_os("APPDATA")?)
                .join("bwquick")
                .join("hello.bin"),
        )
    }

    fn init() {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        }
    }

    fn raise_prompt() {
        std::thread::spawn(|| {
            let until = Instant::now() + Duration::from_secs(5);
            while Instant::now() < until {
                if let Ok(h) = unsafe { FindWindowW(w!("Credential Dialog Xaml Host"), None) } {
                    unsafe {
                        let _ = SetForegroundWindow(h);
                    }
                    return;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        });
    }

    fn status_error(status: KeyCredentialStatus) -> String {
        match status {
            KeyCredentialStatus::UserCanceled => "Windows Hello was cancelled".into(),
            KeyCredentialStatus::NotFound => "Windows Hello key not found".into(),
            KeyCredentialStatus::UserPrefersPassword => {
                "Windows Hello declined, use the master password".into()
            }
            other => format!("Windows Hello failed ({})", other.0),
        }
    }

    fn sign(credential: &KeyCredential) -> Result<Vec<u8>, String> {
        let challenge =
            CryptographicBuffer::CreateFromByteArray(CHALLENGE).map_err(|e| e.to_string())?;
        raise_prompt();
        let result = credential
            .RequestSignAsync(&challenge)
            .and_then(|op| op.join())
            .map_err(|e| e.to_string())?;
        let status = result.Status().map_err(|e| e.to_string())?;
        if status != KeyCredentialStatus::Success {
            return Err(status_error(status));
        }
        let mut bytes = Array::<u8>::new();
        CryptographicBuffer::CopyToByteArray(
            &result.Result().map_err(|e| e.to_string())?,
            &mut bytes,
        )
        .map_err(|e| e.to_string())?;
        Ok(bytes.to_vec())
    }

    pub fn available() -> bool {
        init();
        KeyCredentialManager::IsSupportedAsync()
            .and_then(|op| op.join())
            .unwrap_or(false)
    }

    pub fn enrolled() -> bool {
        file().is_some_and(|f| f.is_file())
    }

    pub fn enroll(secret: &str) -> Result<(), String> {
        init();
        raise_prompt();
        let created = KeyCredentialManager::RequestCreateAsync(
            &HSTRING::from(NAME),
            KeyCredentialCreationOption::ReplaceExisting,
        )
        .and_then(|op| op.join())
        .map_err(|e| e.to_string())?;
        let status = created.Status().map_err(|e| e.to_string())?;
        if status != KeyCredentialStatus::Success {
            return Err(status_error(status));
        }
        let signature = sign(&created.Credential().map_err(|e| e.to_string())?)?;
        let path = file().ok_or("APPDATA is not set")?;
        std::fs::create_dir_all(path.parent().unwrap_or(&path)).map_err(|e| e.to_string())?;
        std::fs::write(&path, sealed::seal(&signature, secret)?).map_err(|e| e.to_string())
    }

    pub fn unlock() -> Result<String, String> {
        init();
        let stored =
            std::fs::read(file().ok_or("APPDATA is not set")?).map_err(|e| e.to_string())?;
        let opened = KeyCredentialManager::OpenAsync(&HSTRING::from(NAME))
            .and_then(|op| op.join())
            .map_err(|e| e.to_string())?;
        let status = opened.Status().map_err(|e| e.to_string())?;
        if status != KeyCredentialStatus::Success {
            return Err(status_error(status));
        }
        let signature = sign(&opened.Credential().map_err(|e| e.to_string())?)?;
        sealed::open(&signature, &stored)
    }

    pub fn forget() {
        if let Some(f) = file() {
            let _ = std::fs::remove_file(f);
        }
        init();
        let _ = KeyCredentialManager::DeleteAsync(&HSTRING::from(NAME)).and_then(|op| op.join());
    }
}

#[cfg(test)]
mod tests {
    use super::sealed::{open, seal};

    #[test]
    fn seal_roundtrip_and_wrong_key() {
        let sealed = seal(b"signature-a", "session-key").unwrap();
        assert_eq!(open(b"signature-a", &sealed).unwrap(), "session-key");
        assert!(open(b"signature-b", &sealed).is_err());
        assert!(open(b"signature-a", &sealed[..10]).is_err());
    }
}
