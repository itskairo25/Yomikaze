import Foundation
import SwiftData
import SwiftUI
import UniformTypeIdentifiers

// merging never deletes anything, the more recent position wins
@MainActor
enum BackupService {
    private static func seconds(_ date: Date) -> Int64 {
        Int64(date.timeIntervalSince1970)
    }

    private static func lines(_ key: String) -> [String] {
        (UserDefaults.standard.string(forKey: key) ?? "").split(separator: "\n").map(String.init)
    }

    static func makeBackup(context: ModelContext) -> BackupFile {
        let saved = (try? context.fetch(FetchDescriptor<SavedManga>())) ?? []
        let progress = (try? context.fetch(FetchDescriptor<ChapterProgress>())) ?? []
        let defaults = UserDefaults.standard

        let manga: [BackupManga] = saved.map { item in
            var entry = BackupManga(mangaId: item.mangaID, title: item.title)
            entry.author = item.author
            entry.status = item.status
            entry.kind = item.type
            entry.inLibrary = item.inLibrary
            entry.dateAdded = seconds(item.dateAdded)
            entry.hasHistory = item.hasHistory
            entry.lastReadAt = item.hasHistory ? seconds(item.lastReadAt) : 0
            entry.lastChapterId = item.lastChapterID
            entry.lastChapterName = item.lastChapterName
            entry.lastPage = item.lastPage
            entry.lastPageCount = item.lastPageCount
            entry.category = item.category
            return entry
        }
        let chapters: [BackupProgress] = progress.map { item in
            var entry = BackupProgress(chapterId: item.chapterID, mangaId: item.mangaID)
            entry.chapterName = item.chapterName
            entry.page = item.page
            entry.pageCount = item.pageCount
            entry.completed = item.completed
            entry.updatedAt = seconds(item.updatedAt)
            return entry
        }

        let settings = BackupSettings(
            accent: defaults.string(forKey: SettingsKey.accent) ?? AccentChoice.yoru.rawValue,
            customAccent: defaults.string(forKey: SettingsKey.customAccent) ?? AppTheme.defaultCustomAccent,
            appearance: defaults.string(forKey: SettingsKey.appearance) ?? AppearanceChoice.system.rawValue,
            readerMode: defaults.string(forKey: SettingsKey.readerMode) ?? ReaderMode.vertical.rawValue,
            tapZones: defaults.object(forKey: SettingsKey.tapZones) as? Bool ?? true,
            readerBackground: defaults.string(forKey: SettingsKey.readerBackground) ?? ReaderBackground.black.rawValue,
            readerDim: defaults.double(forKey: SettingsKey.readerDim),
            readerGap: defaults.bool(forKey: SettingsKey.readerGap),
            keepAwake: defaults.object(forKey: SettingsKey.keepAwake) as? Bool ?? true,
            showAdult: defaults.bool(forKey: SettingsKey.showAdult),
            librarySort: defaults.string(forKey: SettingsKey.librarySort) ?? LibrarySort.recentlyAdded.rawValue
        )

        let version = Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? ""
        return BackupFile(
            app: "ios",
            appVersion: version,
            manga: manga,
            progress: chapters,
            categories: lines(SettingsKey.libraryCategories),
            recentSearches: lines(SettingsKey.recentSearches),
            settings: settings
        )
    }

    @discardableResult
    static func apply(_ file: BackupFile, context: ModelContext, withSettings: Bool) -> (series: Int, chapters: Int) {
        var series = 0
        for incoming in file.manga where !incoming.mangaId.isEmpty {
            series += 1
            let item = context.upsertManga(id: incoming.mangaId, title: incoming.title)
            if item.title.isEmpty {
                item.title = incoming.title
            }
            if item.author.isEmpty { item.author = incoming.author }
            if item.status.isEmpty { item.status = incoming.status }
            if item.type.isEmpty { item.type = incoming.kind }
            if incoming.inLibrary {
                if !item.inLibrary {
                    item.dateAdded = Date(timeIntervalSince1970: TimeInterval(incoming.dateAdded))
                }
                item.inLibrary = true
            }
            if item.category.isEmpty && !incoming.category.isEmpty {
                item.category = incoming.category
            }
            let incomingRead = Date(timeIntervalSince1970: TimeInterval(incoming.lastReadAt))
            if incoming.hasHistory && (!item.hasHistory || incomingRead > item.lastReadAt) {
                item.hasHistory = true
                item.lastReadAt = incomingRead
                item.lastChapterID = incoming.lastChapterId
                item.lastChapterName = incoming.lastChapterName
                item.lastPage = incoming.lastPage
                item.lastPageCount = incoming.lastPageCount
            }
        }

        var chapters = 0
        for incoming in file.progress where !incoming.chapterId.isEmpty {
            chapters += 1
            let updated = Date(timeIntervalSince1970: TimeInterval(incoming.updatedAt))
            if let existing = context.progress(chapterID: incoming.chapterId) {
                let completed = existing.completed || incoming.completed
                if updated > existing.updatedAt {
                    existing.page = incoming.page
                    existing.pageCount = incoming.pageCount
                    existing.updatedAt = updated
                    if !incoming.chapterName.isEmpty {
                        existing.chapterName = incoming.chapterName
                    }
                }
                existing.completed = completed
            } else {
                let item = ChapterProgress(
                    chapterID: incoming.chapterId,
                    mangaID: incoming.mangaId,
                    chapterName: incoming.chapterName,
                    page: incoming.page,
                    pageCount: incoming.pageCount,
                    completed: incoming.completed
                )
                item.updatedAt = updated
                context.insert(item)
            }
        }

        // categories from the backup plus any used by imported series
        var categories = lines(SettingsKey.libraryCategories)
        let used = ((try? context.fetch(FetchDescriptor<SavedManga>())) ?? []).map(\.category).filter { !$0.isEmpty }
        for name in file.categories + used where !name.trimmingCharacters(in: .whitespaces).isEmpty {
            if !categories.contains(where: { $0.caseInsensitiveCompare(name) == .orderedSame }) {
                categories.append(name)
            }
        }
        let defaults = UserDefaults.standard
        defaults.set(categories.joined(separator: "\n"), forKey: SettingsKey.libraryCategories)

        var recent = lines(SettingsKey.recentSearches)
        for term in file.recentSearches where !recent.contains(where: { $0.caseInsensitiveCompare(term) == .orderedSame }) {
            recent.append(term)
        }
        defaults.set(recent.prefix(8).joined(separator: "\n"), forKey: SettingsKey.recentSearches)

        if withSettings, let s = file.settings {
            defaults.set(s.accent, forKey: SettingsKey.accent)
            if Color(hex: s.customAccent) != nil {
                defaults.set(s.customAccent, forKey: SettingsKey.customAccent)
            }
            if AppearanceChoice(rawValue: s.appearance) != nil {
                defaults.set(s.appearance, forKey: SettingsKey.appearance)
            }
            if ReaderMode(rawValue: s.readerMode) != nil {
                defaults.set(s.readerMode, forKey: SettingsKey.readerMode)
            }
            if ReaderBackground(rawValue: s.readerBackground) != nil {
                defaults.set(s.readerBackground, forKey: SettingsKey.readerBackground)
            }
            if LibrarySort(rawValue: s.librarySort) != nil {
                defaults.set(s.librarySort, forKey: SettingsKey.librarySort)
            }
            defaults.set(s.tapZones, forKey: SettingsKey.tapZones)
            defaults.set(min(max(s.readerDim, 0), 0.7), forKey: SettingsKey.readerDim)
            defaults.set(s.readerGap, forKey: SettingsKey.readerGap)
            defaults.set(s.keepAwake, forKey: SettingsKey.keepAwake)
            defaults.set(s.showAdult, forKey: SettingsKey.showAdult)
        }

        try? context.save()
        return (series, chapters)
    }
}

struct BackupDocument: FileDocument {
    static var readableContentTypes: [UTType] { [.json] }

    var data: Data

    init(data: Data) {
        self.data = data
    }

    init(configuration: ReadConfiguration) throws {
        guard let data = configuration.file.regularFileContents else {
            throw BackupError.unreadable
        }
        self.data = data
    }

    func fileWrapper(configuration: WriteConfiguration) throws -> FileWrapper {
        FileWrapper(regularFileWithContents: data)
    }
}
