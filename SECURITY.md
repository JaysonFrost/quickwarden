# Security policy

Quickwarden handles passwords, so security reports are taken seriously.

## Reporting a vulnerability

Please **do not open a public issue**. Use GitHub's private reporting instead: open the repository's **Security** tab and choose **Report a vulnerability**. You will get a reply within a few days.

## Security model

- The vault is read through the official [Bitwarden CLI](https://bitwarden.com/help/cli/). Quickwarden never talks to Bitwarden servers itself and never sees your account credentials beyond the master password you type to unlock.
- The master password is passed to `bw unlock` through a child-process environment variable, never on the command line, and is not stored.
- Decrypted logins live only in the Quickwarden process memory.
- On Windows the vault stays unlocked until restart, like Bitwarden's "On system restart" vault timeout. The `bw` session key is encrypted with DPAPI (bound to your Windows account) and stored in the volatile registry key `HKCU\Software\Quickwarden`, which Windows discards on restart or sign-out. Lock (<kbd>Ctrl</kbd>+<kbd>L</kbd>) deletes it immediately. Trade-off: other programs running under your Windows account could decrypt it while it exists, just as they could read a `BW_SESSION` environment variable.
- `bw serve` is not used, so no local HTTP port exposes the vault.
- Windows Hello: the session key is encrypted with AES-256-GCM. The key is derived (SHA-256) from an RSA signature made by a Windows Hello credential whose private key stays in the TPM and signs only after face, fingerprint or PIN verification. The encrypted blob is stored in `%APPDATA%\quickwarden\hello.bin`.
- Copied secrets are marked to be excluded from clipboard history and cloud clipboard, and are cleared after 30 seconds.
- Autofill types text into the window that was active when the hotkey was pressed. Check the target before pressing Enter, as with any autotype tool.

## Supported versions

Only the latest release receives security fixes.
