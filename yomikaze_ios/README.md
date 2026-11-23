# Yomikaze for iPhone

A manga reader for iPhone built on the [WeebCentral](https://weebcentral.com) catalog.
SwiftUI with Liquid Glass, iOS 26.1 or newer. Everything is stored on the device.

## Features

- Library with your own categories, sorting and a downloads section
- Browse: hot, latest, popular and new, with a featured card and a ranked strip
- Search by title or author, genres, sorting and recent searches
- Series page with AniList banner, facts, progress, synopsis and the chapter list
- Reader: vertical scrolling or paged (left to right or right to left), tap zones, pinch to zoom,
  background colour, dimming and page gaps
- Offline reading of downloaded chapters, plus an offline mode switch
- History and reading statistics
- Backups that can be moved between the iPhone and the Windows app
- Face ID lock, custom accent colour and background image, share cards
- AniList sync: import your manga list, and keep the library, reading progress and favourites in sync
- English, Hungarian, Spanish, Japanese, Chinese and a pirate language

## Project layout

```
project.yml                  XcodeGen project description
.github/workflows/build.yml  CI build of an unsigned .ipa
Tools/SmokeTest/             live check of the WeebCentral client, runs on CI
Yomikaze/
  App/        entry point, tabs, navigation
  Models/     SwiftData models and source types
  Services/   WeebCentral client and parser, image store, downloads, backups
  Design/     theme and shared components
  Features/   Library, Browse, Search, Detail, Reader, History, Stats, Settings
```

If WeebCentral changes its markup, `WeebCentralParser` in `Yomikaze/Services/WeebCentral.swift`
is the only place that needs updating.

## Building

On a Mac with Xcode 26 or newer:

```bash
brew install xcodegen
xcodegen generate
open Yomikaze.xcodeproj
```

Pick your own team under Signing & Capabilities and run it on a device.

Without a Mac, push the project to a GitHub repository: the Build IPA workflow builds an unsigned
`Yomikaze.ipa` on every push to `main`. It can be signed and installed with a sideloading tool
such as Sideloadly or AltStore.

## Data on the device

- SwiftData: library, reading positions, history and downloaded chapter records
- Application Support/Downloads: pages of downloaded chapters
- Application Support/Catalog: cached series details and chapter lists
- Caches/Images: viewed covers and pages
- UserDefaults: settings and recent searches

## AniList

AniList sync needs an API client of your own. Create one at https://anilist.co/settings/developer
with `yomikaze://anilist` as the redirect URL, then put its client ID in `ANILIST_CLIENT_ID` in `project.yml`.

Without a client ID the AniList settings say that AniList isn't set up in this build, and
everything else works as usual.

## AI use

This project was made with help from AI (finding and fixing bugs, translating the app into other
languages, and helping with the parts I couldn't solve on my own).
