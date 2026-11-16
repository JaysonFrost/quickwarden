# bwquick

Quick-access picker for Bitwarden on Windows and macOS, in the spirit of 1Password Quick Access. Press `Ctrl+Shift+Space` anywhere, type to search, press Enter to fill the login into the window you came from. Logins that match the active site or app are marked with ★ and listed first.

Single native binary (Rust, egui). The vault is accessed through the official Bitwarden CLI.

## Requirements

- Bitwarden CLI `bw` in `PATH`: `winget install Bitwarden.CLI`, `brew install bitwarden-cli` or `npm i -g @bitwarden/cli`.
- Log in once in a terminal (handles 2FA and self-hosted servers):

  ```
  bw config server https://your.server   # self-hosted only
  bw login
  ```

## Build

```
cargo build --release
```

Windows: `target\release\bwquick.exe`. To start at login, put a shortcut to it in `shell:startup`.

macOS: build on the Mac and wrap the binary into an app bundle so that privacy permissions stick across launches:

```
./macos-bundle.sh
open target/bwquick.app
```

Then grant `bwquick` **Accessibility** in System Settings → Privacy & Security, and allow it to control System Events and your browser when macOS asks. Add `bwquick.app` to Login Items to start it automatically. After every rebuild run the script again; if typing stops working, remove and re-add the Accessibility entry.

## Keys

| Key | Action |
| --- | --- |
| `Ctrl+Shift+Space` | Open picker (the key combination is reserved by bwquick while it runs) |
| Enter / double-click | Type username, Tab, password into the previous window |
| Shift+Enter | Type password only |
| Ctrl+C / Ctrl+U / Ctrl+T (Cmd on macOS) | Copy password / username / TOTP; cleared after 30 s |
| Ctrl+L (Cmd+L) | Lock vault |
| Ctrl+Q (Cmd+Q) | Quit |
| Esc / window close button | Hide |

## Context matching

The active window is captured when the hotkey is pressed:

- Browser URL: Windows reads the address bar of Chrome, Edge, Brave, Vivaldi, Opera and Firefox through UI Automation; macOS asks Safari and Chromium browsers via AppleScript. A host match (including subdomains) ranks highest.
- Otherwise the window title and process name are matched against each login's domain name (`github` for `github.com`) and item name.

## Security notes

- Unlocking uses `bw unlock --passwordenv`; the master password is passed through the child process environment, never on the command line. The session key and decrypted logins stay in this process's memory only.
- The vault locks after 15 minutes without use, on Ctrl+L and on quit (`bw lock`, which also invalidates other `BW_SESSION` keys).
- `bw serve` is not used, so no local HTTP port exposes the vault.
- Copied secrets are marked to stay out of clipboard history (Windows `Win+V`, cloud clipboard, macOS clipboard managers that honour the concealed type).
- If `BW_SESSION` is set when bwquick starts, it reuses that session.

## Tests

```
cargo test
```

`context::win::reads_browser_url_from_env_window` is ignored by default; run it with `BWQ_TEST_HWND=<browser window handle> BWQ_TEST_APP=msedge cargo test -- --ignored`.
