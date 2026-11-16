use std::path::PathBuf;
use std::process::Command;

use serde::Deserialize;

#[derive(Clone, Debug, Default)]
pub struct Item {
    pub name: String,
    pub username: String,
    pub password: String,
    pub totp: String,
    pub uris: Vec<String>,
}

#[derive(Deserialize)]
struct RawItem {
    #[serde(rename = "type")]
    kind: u8,
    name: Option<String>,
    login: Option<RawLogin>,
}

#[derive(Deserialize)]
struct RawLogin {
    username: Option<String>,
    password: Option<String>,
    totp: Option<String>,
    uris: Option<Vec<RawUri>>,
}

#[derive(Deserialize)]
struct RawUri {
    uri: Option<String>,
}

fn bw_path() -> Option<PathBuf> {
    let names: &[&str] = if cfg!(windows) {
        &["bw.exe", "bw.cmd"]
    } else {
        &["bw"]
    };
    let mut dirs: Vec<PathBuf> = std::env::split_paths(&std::env::var_os("PATH")?).collect();
    if cfg!(target_os = "macos") {
        dirs.extend(["/opt/homebrew/bin", "/usr/local/bin"].map(PathBuf::from));
    }
    dirs.iter()
        .flat_map(|d| names.iter().map(move |n| d.join(n)))
        .find(|p| p.is_file())
}

fn run(session: Option<&str>, args: &[&str], env: &[(&str, &str)]) -> Result<String, String> {
    let bw = bw_path().ok_or("Bitwarden CLI (bw) not found in PATH")?;
    let mut cmd = Command::new(bw);
    cmd.args(args)
        .arg("--nointeraction")
        .envs(env.iter().copied());
    match session {
        Some(s) => cmd.env("BW_SESSION", s),
        None => cmd.env_remove("BW_SESSION"),
    };
    #[cfg(windows)]
    std::os::windows::process::CommandExt::creation_flags(&mut cmd, 0x0800_0000);
    let out = cmd.output().map_err(|e| e.to_string())?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(if out.stderr.is_empty() {
            &out.stdout
        } else {
            &out.stderr
        });
        return Err(err.trim().lines().last().unwrap_or("bw failed").to_string());
    }
    String::from_utf8(out.stdout).map_err(|e| e.to_string())
}

pub fn env_session() -> Option<String> {
    std::env::var("BW_SESSION").ok().filter(|s| !s.is_empty())
}

pub fn status(session: Option<&str>) -> Result<String, String> {
    let v: serde_json::Value =
        serde_json::from_str(&run(session, &["status"], &[])?).map_err(|e| e.to_string())?;
    Ok(v["status"].as_str().unwrap_or("").to_string())
}

pub fn unlock(password: &str) -> Result<String, String> {
    let out = run(
        None,
        &["unlock", "--passwordenv", "BWQ_PASSWORD", "--raw"],
        &[("BWQ_PASSWORD", password)],
    )?;
    Ok(out.trim().to_string())
}

pub fn load(session: &str) -> Result<Vec<Item>, String> {
    run(Some(session), &["sync"], &[])?;
    let raw: Vec<RawItem> = serde_json::from_str(&run(Some(session), &["list", "items"], &[])?)
        .map_err(|e| e.to_string())?;
    Ok(parse(raw))
}

fn parse(raw: Vec<RawItem>) -> Vec<Item> {
    raw.into_iter()
        .filter(|i| i.kind == 1)
        .filter_map(|i| {
            let login = i.login?;
            Some(Item {
                name: i.name.unwrap_or_default(),
                username: login.username.unwrap_or_default(),
                password: login.password.unwrap_or_default(),
                totp: login.totp.unwrap_or_default(),
                uris: login
                    .uris
                    .unwrap_or_default()
                    .into_iter()
                    .filter_map(|u| u.uri)
                    .collect(),
            })
        })
        .collect()
}

pub fn lock(session: &str) {
    let _ = run(Some(session), &["lock"], &[]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_only_logins() {
        let json = r#"[
            {"id":"1","type":1,"name":"GitHub","login":{"username":"me","password":"p","uris":[{"uri":"https://github.com"},{"uri":null}]}},
            {"id":"2","type":2,"name":"Note","login":null}
        ]"#;
        let items = parse(serde_json::from_str(json).unwrap());
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].uris, vec!["https://github.com"]);
    }
}
