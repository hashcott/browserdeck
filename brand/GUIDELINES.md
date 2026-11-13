# Browserdeck logo guidelines

## 1. The logo

**Idea:** a lowercase b whose stem is an arcade joystick (the red ball) and whose bowl holds an arcade button (the yellow cap). Name, control and press in one letter. Colours are the app's Arcade button theme.

| Version | Use it for | Files (`logo/`) |
|---|---|---|
| Horizontal (primary) | README, website header, anywhere with width | `browserdeck-horizontal*.svg` |
| Stacked | Square-ish spaces, splash, merch | `browserdeck-stacked*.svg` |
| Symbol | Avatars, small spaces where the name is nearby | `browserdeck-symbol*.svg` |
| Symbol, small cut | Anything 40 px or smaller | `browserdeck-symbol-small*.svg` |
| Wordmark | Only when the symbol already appears nearby | `browserdeck-wordmark*.svg` |
| App icon | Windows app, tray, phone home screen | `browserdeck-app-icon*.svg` |

Suffixes: none = full colour for light backgrounds · `-reversed` = for dark backgrounds (white b, slightly thinner to stay even) · `-black` / `-white` = one colour.

PNG exports are in `png/`, web icons in `web/`. The app's own icons are `src-tauri/icons/icon.png` and `icon.ico`: the .ico holds the small cut at 16, 20, 24, 32 and 40 px and the regular mark at 48, 64, 128 and 256 px.

## 2. Clear space

Unit **X = the diameter of the red ball**. Keep 1 X clear on every side of any version. In the horizontal lockup the gap between symbol and wordmark is fixed. Don't change it.

## 3. Minimum size

| Version | Screen | Print |
|---|---|---|
| Horizontal | 120 px wide | 30 mm wide |
| Stacked | 64 px wide | 18 mm wide |
| Symbol | 48 px, regular cut | 12 mm |
| Symbol, small cut | 16 px | 5 mm |

Below 48 px always switch to the small cut: thicker b, bigger ball, cap filling the bowl.

## 4. Colour

| Name | HEX | RGB | CMYK (approx.) | Role |
|---|---|---|---|---|
| Arcade Indigo | `#14123A` | 20 18 58 | 66 69 0 77 | The b on light grounds, app icon tile, wordmark |
| Arcade Yellow | `#FFD400` | 255 212 0 | 0 17 100 0 | The button cap |
| Arcade Red | `#FF4040` | 255 64 64 | 0 75 75 0 | The joystick ball |
| Arcade White | `#F4F3FF` | 244 243 255 | 4 5 0 0 | The b on dark grounds |

These are the colours of `themes/arcade.json` (ground, ink, hotkey, OBS scene). The CMYK values are a naive conversion. Proof them on the actual press. No Pantone match has been checked yet. Pick spots from a physical swatch book.

Contrast: white on indigo 16.2:1 · yellow on indigo 12.5:1 · indigo on white 17.8:1 · red on white 3.5:1. Yellow on white is only 1.4:1, so the cap must always sit inside the b's ring and never stand alone on a light ground.

**Approved pairs:** full colour on white or the app's cream (`#F6F1E4`) · reversed on indigo, black or the app's navy (`#2C3A55`) · black on white · white on photos (calm areas only).

The app icon alone adds two depth tints, `#B39500` (cap lip) and `#FFA9A9` (ball gloss), matching the Arcade theme's raised keys. Don't use them anywhere else.

## 5. Typography

Wordmark: **Figtree** ExtraBold (800), tracking −0.02 em, font kerning on, outlined to paths. Figtree is SIL Open Font License 1.1, free for logo use. The app already uses it for all its type. Never retype the wordmark. Use the files.

## 6. Don'ts

- Don't recolour outside the palette, and don't swap yellow and red.
- Don't put the full-colour logo (indigo b) on dark grounds. Use `-reversed`.
- Don't drop the ball or the cap, except in the one-colour small cut (`-small-black`), which keeps the ball and leaves the bowl open.
- Don't use the regular cut below 48 px.
- Don't stretch, rotate, outline, add shadows or gradients, or rearrange the lockup.
- Don't add the gloss or lip tints outside the app icon.
