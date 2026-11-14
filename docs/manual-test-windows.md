# Windows manual test checklist

Run this on real Windows machines before publishing a release. Automated tests cover the logic; these cover what
only a real PC, a real phone and real Windows security features can show.

How to use it: copy the table for the release you are testing (for example into the release's pull request or an
issue), then fill in **Result** (`pass`, `fail` or `n/a`) and **Notes** (Windows build, phone model, what you saw).
A failure in any item except 15 blocks the release.

Setup: a Windows 11 PC and a Windows 10 PC, an iPhone (Safari) and an Android phone (Chrome) on the same WiFi, OBS
Studio 30.2 or newer. The installer always installs for all users (Program Files), which administrator mode needs.

## Install, pairing and buttons

| # | Check | Result | Notes |
|---|---|---|---|
| 1 | Install on a clean Windows 11. SmartScreen warns that the installer is unsigned; that is expected. | | |
| 2 | First start: the pairing screen shows; Windows asks about the firewall; allow it. | | |
| 3 | Scan the QR code with an iPhone (Safari) and an Android phone (Chrome): the button grid opens with no typing. | | |
| 4 | The three volume buttons change the system volume, both with a normal window and with an administrator window focused. | | |
| 5 | Launch button with an `.exe`, and with an `.lnk`. | | |
| 6 | Hotkey button `Ctrl+Shift+Esc` opens Task Manager. | | |
| 7 | Hotkey button while Task Manager (administrator) has focus and the app runs normally: the phone shows the administrator-window error, the desktop shows a toast, no key is sent. | | |
| 8 | Turn administrator mode on: the warning shows, UAC asks, the app restarts. Repeat item 7: the keys arrive. | | |
| 9 | Refuse UAC in item 8: the app keeps running normally and the switch goes back to off. | | |
| 10 | OBS button: OBS closed → the phone says OBS is not connected; start OBS → connected within 5 s, pressing switches the scene; wrong password → Settings says the password is wrong. | | |
| 11 | Start with Windows, normal and administrator mode: sign out and in; the app is in the tray, no UAC prompt, the phone can press buttons. | | |
| 12 | Another program holds port 47821, then start the app: the QR code carries another port and the phone connects. | | |
| 13 | A second phone scans the QR code: the first phone shows the disconnected screen. | | |
| 14 | Simulated power loss: end the process in Task Manager right after editing a button; start again; the configuration is intact. | | |
| 15 | Any fullscreen game: note whether hotkeys arrive (known limitation, does not block a release). | | |
| 16 | Windows 10: repeat items 2, 3, 4 and 6. | | |
| 17 | Switch through all four built-in themes: desktop and phone change at once, buttons stay aligned, labels are readable on every colour, pressed and error states are clearly visible in each. | | |
| 18 | Import `themes/arcade.json` with a changed id and colours: it appears in the list, works, and can be deleted. Import a file with a `css` field: refused with a reason. | | |

## Administrator mode hardening

| # | Check | Result | Notes |
|---|---|---|---|
| 19 | Administrator mode: a launch button opens Notepad → in Task Manager's Details tab, the "Elevated" column says No. | | |
| 20 | A button with "Run as administrator" on: UAC shows, in normal and in administrator mode; accept → Elevated = Yes; refuse → nothing opens. Press from the phone and wait more than 10 s before answering UAC: the phone reports success at once, never a timeout. | | |
| 21 | End `explorer.exe`, press a launch button in administrator mode → an error, nothing opens. Start Explorer again afterwards. | | |
| 22 | `setx WEBVIEW2_BROWSER_EXECUTABLE_FOLDER C:\fake` and `setx WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS --remote-debugging-port=9222`, sign out and in: the administrator-mode app opens normally and port 9222 is not open. Remove both variables afterwards. | | |
| 23 | Administrator mode: the code in the QR code differs from the token in `config.json`; `reg query HKLM\SOFTWARE\Browserdeck\AdminTokens` from a normal command prompt → Access is denied. | | |
| 24 | (Merged into item 31.) | | |
| 25 | Administrator mode: open `http://127.0.0.1:<port>/?token=<code from the QR>` on the PC itself → 403; the phone still connects. | | |
| 26 | While the administrator-mode app runs: in a normal PowerShell, `$l = New-Object Net.Sockets.TcpListener('127.0.0.1', <port>); $l.Start()` → "address already in use". | | |
| 27 | Windows 11: close the app, move `%APPDATA%\com.browserdeck.desktop` away, `mklink /J` the same name to a folder only administrators can write; start in administrator mode and edit a button → a save error, and no file appears in the target folder. Undo the junction afterwards. | | |
| 28 | Administrator mode: Process Explorer, DLLs tab of browserdeck.exe → `EmbeddedBrowserWebView.dll` is under `Program Files (x86)\Microsoft\EdgeWebView`. | | |
| 29 | Normal mode, phone connected: quit from the tray and start again at once → the QR code keeps the same port and the phone reconnects by itself. | | |
| 30 | Administrator mode: `<install folder>\WebView2Admin` exists; Process Explorer, msedgewebview2.exe → its TEMP variable points to `WebView2Admin\Temp`. | | |
| 31 | Uninstall: `HKLM\SOFTWARE\Browserdeck`, `<install folder>\WebView2Admin`, the `browserdeck-autostart` task and the `Browserdeck` value under `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` are gone. | | |

## Installer

| # | Check | Result | Notes |
|---|---|---|---|
| 32 | With administrator mode and start with Windows on, run the next version's installer and keep "Uninstall before installing": the phone does not need to scan again, and the app still starts at sign-in. Then uninstall from Settings > Apps: item 31 holds. | | |
| 33 | Uninstall from Settings > Apps with administrator mode never turned on: no error, the install folder is gone. | | |
