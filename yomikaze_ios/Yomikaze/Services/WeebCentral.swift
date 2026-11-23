import Foundation

enum SourceError: LocalizedError {
    case badStatus(Int)
    case invalidResponse
    case empty

    var errorDescription: String? {
        switch self {
        case .badStatus(let code):
            return "WeebCentral answered with an error (HTTP \(code))."
        case .invalidResponse:
            return "Couldn't understand the response from WeebCentral."
        case .empty:
            return "Nothing was found here."
        }
    }
}

enum OfflineMode {
    static let key = "offlineMode"

    static var isForced: Bool {
        UserDefaults.standard.bool(forKey: key)
    }
}

enum SearchSort: String, CaseIterable, Identifiable, Sendable {
    case bestMatch = "Best Match"
    case popularity = "Popularity"
    case latestUpdates = "Latest Updates"
    case recentlyAdded = "Recently Added"
    case subscribers = "Subscribers"
    case alphabet = "Alphabet"

    var id: String { rawValue }
}

struct WeebCentral: Sendable {
    static let shared = WeebCentral()

    static let base = "https://weebcentral.com"
    static let pageSize = 32
    static let userAgent = "Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Mobile/15E148 Safari/604.1"

    private let session: URLSession

    init() {
        let configuration = URLSessionConfiguration.default
        configuration.httpAdditionalHeaders = [
            "User-Agent": WeebCentral.userAgent,
            "Referer": WeebCentral.base + "/",
            "Accept-Language": "en-US,en;q=0.9",
        ]
        configuration.timeoutIntervalForRequest = 30
        configuration.waitsForConnectivity = false
        session = URLSession(configuration: configuration)
    }

    // small is the 200x300 version
    static func coverURL(_ mangaID: String, small: Bool = false) -> URL {
        URL(string: "https://temp.compsci88.com/cover/\(small ? "small" : "normal")/\(mangaID).webp")!
    }

    static func seriesURL(_ mangaID: String) -> URL {
        URL(string: "\(base)/series/\(mangaID)")!
    }

    // empty text with .popularity gives the most popular list
    func search(
        text: String,
        sort: SearchSort,
        tag: String? = nil,
        offset: Int = 0,
        includeAdult: Bool = false
    ) async throws -> ResultPage {
        var components = URLComponents(string: "\(WeebCentral.base)/search/data")!
        var items: [URLQueryItem] = [
            URLQueryItem(name: "text", value: text),
            URLQueryItem(name: "sort", value: sort.rawValue),
            URLQueryItem(name: "order", value: sort == .alphabet ? "Ascending" : "Descending"),
            URLQueryItem(name: "official", value: "Any"),
            URLQueryItem(name: "anime", value: "Any"),
            URLQueryItem(name: "adult", value: includeAdult ? "Any" : "False"),
            URLQueryItem(name: "display_mode", value: "Full Display"),
            URLQueryItem(name: "limit", value: String(WeebCentral.pageSize)),
            URLQueryItem(name: "offset", value: String(offset)),
        ]
        if let tag, !tag.isEmpty {
            items.append(URLQueryItem(name: "included_tag", value: tag))
        }
        components.queryItems = items
        guard let url = components.url else { throw SourceError.invalidResponse }

        let html = try await fetchHTML(url)
        let hasMore = html.contains("offset=\(offset + WeebCentral.pageSize)")
        return ResultPage(items: WeebCentralParser.cards(html), hasMore: hasMore)
    }

    func latestUpdates(page: Int = 1) async throws -> ResultPage {
        let html = try await fetchHTML(URL(string: "\(WeebCentral.base)/latest-updates/\(page)")!)
        let hasMore = html.contains("latest-updates/\(page + 1)")
        return ResultPage(items: WeebCentralParser.cards(html), hasMore: hasMore)
    }

    func hotUpdates() async throws -> [MangaSummary] {
        let html = try await fetchHTML(URL(string: "\(WeebCentral.base)/hot-updates")!)
        return WeebCentralParser.cards(html)
    }

    func detail(mangaID: String) async throws -> MangaDetail {
        let html = try await fetchHTML(WeebCentral.seriesURL(mangaID))
        guard let detail = WeebCentralParser.detail(html, mangaID: mangaID) else {
            throw SourceError.invalidResponse
        }
        return detail
    }

    func chapters(mangaID: String) async throws -> [Chapter] {
        let html = try await fetchHTML(URL(string: "\(WeebCentral.base)/series/\(mangaID)/full-chapter-list")!)
        return WeebCentralParser.chapters(html)
    }

    func pages(chapterID: String) async throws -> [URL] {
        let url = URL(string: "\(WeebCentral.base)/chapters/\(chapterID)/images?is_prev=False&current_page=1&reading_style=long_strip")!
        let html = try await fetchHTML(url)
        let pages = WeebCentralParser.pages(html)
        guard !pages.isEmpty else { throw SourceError.empty }
        return pages
    }

    private func fetchHTML(_ url: URL) async throws -> String {
        if OfflineMode.isForced {
            throw URLError(.notConnectedToInternet)
        }
        let (data, response) = try await session.data(from: url)
        guard let http = response as? HTTPURLResponse else {
            throw SourceError.invalidResponse
        }
        guard (200..<300).contains(http.statusCode) else {
            throw SourceError.badStatus(http.statusCode)
        }
        return String(decoding: data, as: UTF8.self)
    }
}

// if the site changes its markup this is the only place to update
enum WeebCentralParser {
    private static let idPattern = "[0-9A-Z]{26}"

    private static let fractionalDateFormatter: ISO8601DateFormatter = {
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        return formatter
    }()

    private static let plainDateFormatter: ISO8601DateFormatter = {
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withInternetDateTime]
        return formatter
    }()

    static func date(_ string: String?) -> Date? {
        guard let string, !string.isEmpty else { return nil }
        return fractionalDateFormatter.date(from: string) ?? plainDateFormatter.date(from: string)
    }

    // every card is an <article>, duplicates for the same series are merged
    static func cards(_ html: String) -> [MangaSummary] {
        var order: [String] = []
        var byID: [String: MangaSummary] = [:]

        for chunk in html.components(separatedBy: "<article").dropFirst() {
            let text = RegexText(chunk)
            let seriesID = text.first("/series/(\(idPattern))")
                ?? text.first("/cover/[a-z]+/(\(idPattern))\\.")
            guard let id = seriesID else { continue }

            var manga = byID[id] ?? MangaSummary(id: id, title: "")
            if byID[id] == nil {
                order.append(id)
            }

            if manga.title.isEmpty {
                let title = text.first(#"^[^>]*data-tip="([^"]*)""#)
                    ?? text.first(#"alt="([^"]*?) cover""#)
                if let title {
                    manga.title = title.cleanedText
                }
            }
            if manga.latestChapter == nil {
                manga.latestChapter = text.first(#"\.svg"[^>]*>\s*<span>([^<]+)</span>"#)?.cleanedText
            }
            if manga.latestChapterID == nil {
                manga.latestChapterID = text.first("/chapters/(\(idPattern))")
            }
            if manga.updatedAt == nil {
                manga.updatedAt = date(text.first(#"datetime="([^"]+)""#))
            }
            if manga.year == nil {
                manga.year = text.first(#"<strong>Year:</strong>\s*<span>([^<]*)</span>"#)?.cleanedText
            }
            if manga.status == nil {
                manga.status = text.first(#"<strong>Status:</strong>\s*<span>([^<]*)</span>"#)?.cleanedText
            }
            if manga.type == nil {
                manga.type = text.first(#"<strong>Type:</strong>\s*<span>([^<]*)</span>"#)?.cleanedText
            }

            byID[id] = manga
        }

        return order.compactMap { byID[$0] }.filter { !$0.title.isEmpty }
    }

    static func detail(_ html: String, mangaID: String) -> MangaDetail? {
        guard let title = html.firstCapture(#"<h1[^>]*>([^<]+)</h1>"#)?.cleanedText, !title.isEmpty else {
            return nil
        }

        func field(_ label: String) -> [String] {
            guard let block = html.firstCapture("<strong>\\s*\(label)[^<]*</strong>([\\s\\S]*?)</li>") else {
                return []
            }
            var values = block.allCaptures(#"<a[^>]*>([^<]+)</a>"#).compactMap { $0.first }
            if values.isEmpty {
                values = block.allCaptures(#"<span[^>]*>([^<]+)</span>"#).compactMap { $0.first }
            }
            return values.map { $0.cleanedText }.filter { !$0.isEmpty }
        }

        let rawSummary = html.firstCapture(#"<strong>Description</strong>\s*<p[^>]*>([\s\S]*?)</p>"#) ?? ""
        let summary = rawSummary
            .replacingOccurrences(of: "<br>", with: "\n")
            .replacingOccurrences(of: "<br/>", with: "\n")
            .replacingOccurrences(of: "<br />", with: "\n")
            .htmlDecoded
            .trimmingCharacters(in: .whitespacesAndNewlines)

        return MangaDetail(
            id: mangaID,
            title: title,
            authors: field("Author"),
            tags: field("Tag"),
            type: field("Type").first ?? "",
            status: field("Status").first ?? "",
            released: field("Released").first ?? "",
            official: field("Official Translation").first == "Yes",
            adult: field("Adult Content").first == "Yes",
            summary: summary,
            anilistID: html.firstCapture(#"anilist\.co/manga/(\d+)"#).flatMap { Int($0) }
        )
    }

    static func chapters(_ html: String) -> [Chapter] {
        let pattern = #"href="(?:https://weebcentral\.com)?/chapters/([0-9A-Z]{26})"[^>]*>[\s\S]*?<span class="grow[^"]*">\s*<span[^>]*>([^<]*)</span>[\s\S]*?datetime="([^"]+)""#
        var seen = Set<String>()
        var chapters: [Chapter] = []
        for groups in html.allCaptures(pattern) where groups.count == 3 {
            let id = groups[0]
            guard seen.insert(id).inserted else { continue }
            chapters.append(Chapter(id: id, name: groups[1].cleanedText, date: date(groups[2])))
        }
        return chapters
    }

    static func pages(_ html: String) -> [URL] {
        html.allCaptures(#"<img\s[^>]*?src="(https?://[^"]+)""#)
            .compactMap { $0.first }
            .compactMap { URL(string: $0.htmlDecoded) }
    }
}
