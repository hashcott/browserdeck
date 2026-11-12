# Button themes

A theme is a small JSON data file that styles the buttons and the page behind them. **A theme contains no CSS, no code, no file paths and no URLs.** Every field is either a `#rrggbb` colour, a bounded number, or one of a fixed list of keywords; anything else is rejected when the file is imported, and unknown fields are rejected too. The app turns these values into styles itself.

## Fields

| Field | Type | Allowed values |
|---|---|---|
| `schema` | number | Must be `1`. |
| `id` | string | 1 to 40 characters of `a-z`, `0-9` and `-`. Used as the file name, so it must be unique. |
| `name` | string | 1 to 40 characters, not blank. Shown in the theme picker. |
| `author` | string | 0 to 40 characters. May be empty. |
| `ground` | colour | Background of the phone page and of the grid area on the desktop. |
| `ink` | colour | Text colour on that background. |
| `actions.launch` | colour | Default button colour for "launch an app" buttons. |
| `actions.hotkey` | colour | Default colour for hotkey buttons. |
| `actions.volume` | colour | Default colour for volume buttons. |
| `actions.obs_scene` | colour | Default colour for OBS scene buttons. |
| `key.radius` | number | 0 to 50. Corner rounding, in percent of the button side. |
| `key.gap` | number | 0 to 20. Space between buttons, in percent of the button side. |
| `key.border_width` | number | 0 to 8. Border thickness in pixels. `0` means no border. |
| `key.border_color` | colour | Border colour. Required even when the width is 0. |
| `key.depth` | keyword | `flat`, `dish`, `raised` or `sticker`. |
| `key.press` | keyword | `sink`, `dim` or `lift`. What the button does while pressed. |
| `key.legend` | keyword | `center`, `top_left` or `bottom_left`. Where the label sits. |

A colour is exactly `#` followed by six hex digits (`#22252b`). Short forms (`#fff`), colour names (`red`) and functions (`rgb(...)`, `url(...)`) are not accepted. Numbers must be whole numbers inside their range.

The file may be at most 64 KiB.

## Example

```json
{
  "schema": 1,
  "id": "midnight",
  "name": "Midnight",
  "author": "Your Name",
  "ground": "#101820",
  "ink": "#f2f2f2",
  "actions": {
    "launch": "#4f6d8f",
    "hotkey": "#7a5c99",
    "volume": "#4f8f6d",
    "obs_scene": "#d9822b"
  },
  "key": {
    "radius": 12,
    "gap": 8,
    "border_width": 2,
    "border_color": "#000000",
    "depth": "raised",
    "press": "sink",
    "legend": "center"
  }
}
```

## Importing a theme

Open the app's settings, choose the theme section, press "Import theme" and pick the `.json` file. The app checks it and, if it is valid, saves a normalised copy into its own `themes` folder and adds it to the list. Importing a theme with the id of an earlier import replaces that one. The ids of the built-in themes (`keycap`, `sticker`, `arcade`, `flat`) cannot be used. Imported themes can be deleted from the same list; built-in ones cannot.

If a selected theme is later missing or damaged, the app falls back to `keycap`.
