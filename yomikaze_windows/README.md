# Yomikaze for Windows

The Windows version of the Yomikaze manga reader, written in Rust with egui. It has the same
features as the iPhone app and reads the same backup files.

## Features

- Library with your own categories, sorting and a downloads section
- Browse: hot, latest, popular and new, with a featured card and a ranked strip
- Search by title or author, genres, sorting and recent searches
- Series page with AniList banner, facts, progress, synopsis and the chapter list
- Reader: vertical scrolling or paged (left to right or right to left), two pages side by side,
  a chapter list, adjustable page width, tap zones, keyboard and mouse shortcuts, zoom,
  background colour, dimming and page gaps. The next chapter loads before you reach it.
- Offline reading of downloaded chapters, plus an offline mode switch
- History and reading statistics, backups, share cards
- Windows Hello lock, custom accent colour and background image, with a gallery of premade
  backgrounds
- AniList sync: import your manga list, and keep the library, reading progress and favourites in sync
- English, Hungarian, Spanish, Japanese, Chinese and a pirate language

## Reader keys

`Left` `Right` or `A` `D` turn pages, `Space` and `PgDn` scroll (`Shift` + `Space` goes back),
holding `Up` `Down` or `W` `S` scrolls smoothly, `Home` and `End` jump, `T` two pages,
`C` chapter list, `F` or `F11` full screen, `Tab` shows the controls, `Esc` closes.
`Ctrl` + wheel zooms a page, or changes the page width in vertical mode. The mouse back and
forward buttons turn pages too. Moving the mouse to the top or bottom edge shows the controls,
and the cursor hides while you read.

## Building

```bash
cargo build --release
```

The exe is `target\release\yomikaze.exe`. It links the C runtime statically, so it runs on any
Windows 10 or 11 machine without extra installs.

## Installer

Needs [Inno Setup 6](https://jrsoftware.org/isinfo.php).

```bash
cargo build --release
"%LOCALAPPDATA%\Programs\Inno Setup 6\ISCC.exe" installer\yomikaze.iss
```

The installer goes to `..\yomikaze_windows_build`. It installs per user into
`%LOCALAPPDATA%\Programs\Yomikaze` without admin rights, and updates an existing install.

## Data

- `%APPDATA%\Yomikaze`: library, history, settings, downloaded chapters and picked backgrounds
- `%LOCALAPPDATA%\Yomikaze`: image cache
- `Pictures\Yomikaze`: saved share cards

Installing, updating or removing the app never touches this data.

## Development switches

- `YOMIKAZE_DATA_DIR` keeps all data in another folder, so tests never touch your real library
- `YOMIKAZE_START` opens a screen directly, for example `browse`, `settings` or `detail:<series id>`
- `YOMIKAZE_SIZE` sets the starting window size, for example `1100x650`
- `YOMIKAZE_READER` opens the reader with panels showing, for example `chrome,chapters` or `settings`

## Premade backgrounds

Settings > Backgrounds has anime wallpapers from the Wallhaven API: only the anime category, only
safe for work, at least 1920x1080 and widescreen. The popular tab is the most liked ones of the
past year. The genre tabs list each genre's best known series from AniList (kept in
`anime_backgrounds.json` for a week) and show wallpapers of the picked series. No pictures are part
of this repository, they belong to their owners.

The gallery only loads previews. A picture is downloaded when you pick it and kept at up to 4K in
`%APPDATA%\Yomikaze\Wallpapers`, so it still works offline. The code is in `src/wallpapers.rs`.

## Icons

Most icons are Yomikaze's own, drawn in code by `tools/make_icons.py` (it needs `fonttools`) into
`assets/yomikaze-icons.ttf`. `src/icons.rs` maps them, and anything not drawn there comes from
Phosphor.

## AniList

AniList sync needs an API client of your own. Create one at https://anilist.co/settings/developer
with `yomikaze://anilist` as the redirect URL, then build with its client ID in the `YOMIKAZE_ANILIST_CLIENT_ID`
environment variable:

```powershell
$env:YOMIKAZE_ANILIST_CLIENT_ID = "12345"
cargo build --release
```

Without a client ID the AniList settings say that AniList isn't set up in this build, and
everything else works as usual.

## AI use

This project was made with help from AI (finding and fixing bugs, translating the app into other
languages, and helping with the parts I couldn't solve on my own).
