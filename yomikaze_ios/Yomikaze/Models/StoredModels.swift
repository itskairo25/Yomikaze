import Foundation
import SwiftData

// persisted with swiftdata

@Model
final class SavedManga {
    @Attribute(.unique) var mangaID: String
    var title: String
    var author: String
    var status: String
    var type: String

    var inLibrary: Bool
    var dateAdded: Date

    var hasHistory: Bool
    var lastReadAt: Date
    var lastChapterID: String
    var lastChapterName: String
    var lastPage: Int
    var lastPageCount: Int
    // empty means no category, the default lets older stores migrate automatically
    var category: String = ""
    var anilistID: Int? = nil
    // chapters read according to anilist, applied when the chapter list is loaded
    var anilistProgress: Int? = nil

    init(mangaID: String, title: String) {
        self.mangaID = mangaID
        self.title = title
        self.author = ""
        self.status = ""
        self.type = ""
        self.inLibrary = false
        self.dateAdded = Date()
        self.hasHistory = false
        self.lastReadAt = Date.distantPast
        self.lastChapterID = ""
        self.lastChapterName = ""
        self.lastPage = 0
        self.lastPageCount = 0
    }

    var summary: MangaSummary {
        MangaSummary(
            id: mangaID,
            title: title,
            status: status.isEmpty ? nil : status,
            type: type.isEmpty ? nil : type
        )
    }
}

@Model
final class ChapterProgress {
    @Attribute(.unique) var chapterID: String
    var mangaID: String
    var chapterName: String
    var page: Int
    var pageCount: Int
    var completed: Bool
    var updatedAt: Date

    init(chapterID: String, mangaID: String, chapterName: String, page: Int, pageCount: Int, completed: Bool) {
        self.chapterID = chapterID
        self.mangaID = mangaID
        self.chapterName = chapterName
        self.page = page
        self.pageCount = pageCount
        self.completed = completed
        self.updatedAt = Date()
    }
}

@Model
final class DownloadedChapter {
    @Attribute(.unique) var chapterID: String
    var mangaID: String
    var mangaTitle: String
    var chapterName: String
    var pageURLs: [String]
    var downloadedAt: Date

    init(chapterID: String, mangaID: String, mangaTitle: String, chapterName: String, pageURLs: [String]) {
        self.chapterID = chapterID
        self.mangaID = mangaID
        self.mangaTitle = mangaTitle
        self.chapterName = chapterName
        self.pageURLs = pageURLs
        self.downloadedAt = Date()
    }
}

extension ModelContext {
    func savedManga(_ id: String) -> SavedManga? {
        var descriptor = FetchDescriptor<SavedManga>(predicate: #Predicate<SavedManga> { $0.mangaID == id })
        descriptor.fetchLimit = 1
        return (try? fetch(descriptor))?.first
    }

    func upsertManga(id: String, title: String) -> SavedManga {
        if let existing = savedManga(id) {
            return existing
        }
        let manga = SavedManga(mangaID: id, title: title)
        insert(manga)
        return manga
    }

    func progress(chapterID: String) -> ChapterProgress? {
        var descriptor = FetchDescriptor<ChapterProgress>(predicate: #Predicate<ChapterProgress> { $0.chapterID == chapterID })
        descriptor.fetchLimit = 1
        return (try? fetch(descriptor))?.first
    }

    func downloadedChapter(_ chapterID: String) -> DownloadedChapter? {
        var descriptor = FetchDescriptor<DownloadedChapter>(predicate: #Predicate<DownloadedChapter> { $0.chapterID == chapterID })
        descriptor.fetchLimit = 1
        return (try? fetch(descriptor))?.first
    }

    // also moves the manga to the top of the history
    func recordProgress(mangaID: String, mangaTitle: String, chapter: Chapter, page: Int, pageCount: Int, completed: Bool) {
        let manga = upsertManga(id: mangaID, title: mangaTitle)
        manga.hasHistory = true
        manga.lastReadAt = Date()
        manga.lastChapterID = chapter.id
        manga.lastChapterName = chapter.name
        manga.lastPage = page
        manga.lastPageCount = pageCount

        if let existing = progress(chapterID: chapter.id) {
            let finished = completed && !existing.completed
            existing.page = page
            existing.pageCount = pageCount
            existing.completed = existing.completed || completed
            existing.updatedAt = Date()
            if finished {
                AniListSync.changed(mangaID)
            }
        } else {
            insert(ChapterProgress(
                chapterID: chapter.id,
                mangaID: mangaID,
                chapterName: chapter.name,
                page: page,
                pageCount: pageCount,
                completed: completed
            ))
            // a new chapter is worth telling anilist about
            AniListSync.changed(mangaID)
        }
    }

    // it stays stored while it is still in the library
    func removeFromHistory(_ manga: SavedManga) {
        let id = manga.mangaID
        try? delete(model: ChapterProgress.self, where: #Predicate<ChapterProgress> { $0.mangaID == id })
        if manga.inLibrary {
            manga.hasHistory = false
            manga.lastReadAt = Date.distantPast
            manga.lastChapterID = ""
            manga.lastChapterName = ""
            manga.lastPage = 0
            manga.lastPageCount = 0
        } else {
            delete(manga)
        }
    }
}
