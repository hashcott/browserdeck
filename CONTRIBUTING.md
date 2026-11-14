# Contributing to Browserdeck

Thanks for helping. Bug reports, ideas, translations, themes and code are all welcome.

## Before you start

- **Bugs and ideas:** open an [issue](https://github.com/hashcott/browserdeck/issues/new/choose). For a bigger change, open an issue first so we can agree on the approach before you write code.
- **Security problems:** do not open a public issue. See [SECURITY.md](SECURITY.md).
- Browserdeck keeps a small scope on purpose: four button types, local network only, one phone. Ideas outside that may be declined even when they are good ideas.

## Development setup

You need Node.js 22, Rust (stable) and the [Tauri prerequisites](https://tauri.app/start/prerequisites/). Windows is the target platform; the app also builds and runs on macOS for development, where the Windows-only parts (volume, hotkeys, administrator mode) report "unsupported".

```bash
npm ci
npm run tauri dev
```

The desktop window and the phone page are React + TypeScript (`src/desktop`, `src/phone`, `src/shared`). The core is Rust (`src-tauri/src`): the phone server, actions, configuration and Windows integration.

## Checks

Run all three before opening a pull request; CI runs them on Windows and macOS:

```bash
npm test -- --run
cargo test --manifest-path src-tauri/Cargo.toml
npm run build
```

New behaviour needs a test that fails without it. Windows-only code that cannot be tested automatically gets a step in [docs/manual-test-windows.md](docs/manual-test-windows.md).

## Guidelines

- **Text:** every visible string goes through `src/shared/locales/en.json` and `vi.json`. Both files must have the same keys (a test checks this). If you cannot write Vietnamese, add the English text to both and say so in the pull request.
- **Commits:** [Conventional Commits](https://www.conventionalcommits.org/) style, for example `feat: page reorder`, `fix: phone reconnect after sleep`, `docs: themes`. Note user-facing changes in the top section of [CHANGELOG.md](CHANGELOG.md); release notes are written from it.
- **Security-sensitive code** (the phone server, pairing, administrator mode in `elevation.rs`, `admin_token.rs`, `actions/launch.rs`): explain the threat you considered in the pull request.
- **Themes:** a theme is data, not code. See [docs/themes.md](docs/themes.md); new built-in themes go in `themes/`.

## License

Browserdeck is licensed under [GPL-3.0](LICENSE). By contributing, you agree that your contribution is licensed under GPL-3.0 as well.
