import Foundation

struct MangaSummary: Identifiable, Hashable, Codable, Sendable {
    let id: String
    var title: String
    var status: String? = nil
    var type: String? = nil
    var year: String? = nil
    var latestChapter: String? = nil
    var latestChapterID: String? = nil
    var updatedAt: Date? = nil

    var metaLine: String {
        [type, status, year]
            .compactMap { $0 }
            .filter { !$0.isEmpty }
            .joined(separator: " · ")
    }
}

struct MangaDetail: Hashable, Codable, Sendable {
    var id: String
    var title: String
    var authors: [String]
    var tags: [String]
    var type: String
    var status: String
    var released: String
    var official: Bool
    var adult: Bool
    var summary: String
    // anilist entry linked from the series page, used for artwork
    var anilistID: Int? = nil
}

struct Chapter: Identifiable, Hashable, Codable, Sendable {
    let id: String
    var name: String
    var date: Date? = nil
}

struct ResultPage: Sendable {
    var items: [MangaSummary]
    var hasMore: Bool
}

struct ReaderRequest: Identifiable {
    let id = UUID()
    let mangaID: String
    let mangaTitle: String
    // newest first, may be empty, the reader loads it on demand
    var chapters: [Chapter]
    var chapter: Chapter
}
