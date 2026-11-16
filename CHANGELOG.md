# Changelog

All notable changes to Browserdeck are listed here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow [Semantic Versioning](https://semver.org/).

## [0.1.0] - Unreleased

First public release.

### Added

- Phone page: a grid of buttons in the phone's browser, paired by scanning a QR code that carries a secret code. One phone at a time; a second phone takes over and the first one is told.
- The phone keeps working when the PC gets a new IP: the pairing screen also offers the PC's `.local` name (iPhone, Android 12 or newer), and an open phone page notices a dead link within 35 seconds and connects again. The phone page can be added to the iPhone home screen.
- Six button types: open an app (with arguments; choosing the program gives the key its icon), send a keyboard shortcut, change the system volume (up, down, mute), media keys (previous, play/pause, next; dropping Media on a slot with room makes all three in a row), switch OBS scene (OBS 30.2 or newer, through its built-in WebSocket server), and system usage: the key shows CPU, RAM or temperature, updated every 2 seconds (temperature only where the PC exposes a sensor).
- Desktop editor: drag actions onto the grid, drag buttons to reorder, set label, icon and colour. Icons: Phosphor icons and Simple Icons app logos built in (searchable; picking a logo gives the key its brand colour), an emoji or an image. New keys start with an icon that fits their action. Several pages, as tabs, each with its own theme and grid size or the defaults. Edits reach the phone at once.
- Four built-in button themes and importable JSON themes ([docs/themes.md](docs/themes.md)).
- English and Vietnamese, following the system and browser language or chosen in Settings.
- System tray, start with Windows, first-run pairing screen.
- Administrator mode, so shortcut buttons reach apps running as administrator, with safeguards: launched apps still run normally unless a button asks for "Run as administrator" (UAC every time), the pairing code is kept where only administrators can read it, only phones on the local network are accepted, and the elevated app ignores WebView2 and DLL search settings from the user's environment.
- Windows installer (NSIS, x64) built and attached to GitHub Releases with SHA-256 checksums.

[0.1.0]: https://github.com/hashcott/browserdeck/releases/tag/v0.1.0
