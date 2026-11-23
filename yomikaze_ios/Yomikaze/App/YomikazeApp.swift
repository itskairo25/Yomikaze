import SwiftData
import SwiftUI

@main
struct YomikazeApp: App {
    @AppStorage(SettingsKey.accent) private var accentRaw = AccentChoice.yoru.rawValue
    @AppStorage(SettingsKey.customAccent) private var customAccentHex = AppTheme.defaultCustomAccent
    @AppStorage(SettingsKey.appearance) private var appearanceRaw = AppearanceChoice.system.rawValue

    var body: some Scene {
        WindowGroup {
            RootView()
                .tint(AppTheme.accent(raw: accentRaw, customHex: customAccentHex))
                .preferredColorScheme((AppearanceChoice(rawValue: appearanceRaw) ?? .system).colorScheme)
        }
        .modelContainer(for: [SavedManga.self, ChapterProgress.self, DownloadedChapter.self])
    }
}
