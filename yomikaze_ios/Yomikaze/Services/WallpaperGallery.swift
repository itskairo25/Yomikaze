import CryptoKit
import Foundation
import Observation
import SwiftUI
import UIKit

// anime wallpapers from wallhaven: only the anime category and only safe for work. an iphone
// gets tall pictures that fill the portrait screen, an ipad wide ones. the gallery only shows
// small previews, a picture is downloaded when it's picked and kept for offline use.
struct Wallpaper: Identifiable, Hashable {
    let full: URL
    let thumb: URL
    let resolution: String

    var id: URL { full }
}

@MainActor
@Observable
final class WallpaperGallery {
    static let shared = WallpaperGallery()

    static let genres = [
        "Action", "Adventure", "Comedy", "Drama", "Fantasy", "Romance", "Sci-Fi", "Slice of Life",
        "Sports", "Mystery", "Horror", "Supernatural", "Mecha", "Music", "Psychological", "Thriller",
    ]

    // hand picked famous series, written the way wallpaper sites tag them
    static let famous = [
        "Attack on Titan", "Demon Slayer", "Jujutsu Kaisen", "One Piece", "Naruto", "Bleach", "Dragon Ball",
        "Death Note", "Fullmetal Alchemist", "Hunter x Hunter", "My Hero Academia", "Chainsaw Man", "Spy x Family",
        "Frieren", "Solo Leveling", "Tokyo Ghoul", "One Punch Man", "Mob Psycho 100", "Sword Art Online", "Re:Zero",
        "Steins;Gate", "Code Geass", "Neon Genesis Evangelion", "Cowboy Bebop", "Your Name", "Weathering with You",
        "A Silent Voice", "Spirited Away", "Howl's Moving Castle", "Princess Mononoke", "Your Lie in April",
        "Violet Evergarden", "The Quintessential Quintuplets", "Kaguya-sama", "Rent-a-Girlfriend", "Horimiya",
        "Toradora", "Oshi no Ko", "Bocchi the Rock", "K-On", "My Dress-Up Darling", "Bunny Girl Senpai",
        "Lycoris Recoil", "Nisekoi", "Clannad", "Angel Beats", "Haikyuu", "Blue Lock", "Kuroko no Basket",
        "Vinland Saga", "Berserk", "JoJo's Bizarre Adventure", "Hellsing", "Black Clover", "Fire Force", "Dr. Stone",
        "The Promised Neverland", "Akame ga Kill", "Darling in the Franxx", "Kill la Kill", "Gurren Lagann",
        "Cyberpunk Edgerunners", "Made in Abyss", "Konosuba", "Overlord", "Tensei Shitara Slime Datta Ken",
        "Mushoku Tensei", "The Rising of the Shield Hero", "Fate", "Dandadan", "Kaiju No. 8", "Sakamoto Days",
        "Bungo Stray Dogs", "Noragami", "Assassination Classroom", "Tokyo Revengers", "Hell's Paradise",
        "Erased", "Parasyte", "Psycho-Pass", "Monogatari", "Ghost in the Shell", "Akira", "Boruto",
    ]

    struct Search {
        var items: [Wallpaper] = []
        var page = 0
        var lastPage = 1
        var loading = false
        var error: String?

        var hasMore: Bool { page < lastPage }
    }

    // wallhaven results by search text, "" is the most liked of the past year
    private(set) var searches: [String: Search] = [:]
    // the best known series of each genre, in the order of genres
    private(set) var genreSeries: [[String]]?
    private(set) var seriesError: String?
    private(set) var loadingSeries = false
    // a picked picture that is downloading right now
    private(set) var busy: URL?

    private let session: URLSession
    private let folder: URL

    private init() {
        let configuration = URLSessionConfiguration.default
        let version = Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? "0"
        configuration.httpAdditionalHeaders = ["User-Agent": "Yomikaze/\(version) (manga reader for iOS)"]
        configuration.timeoutIntervalForRequest = 60
        session = URLSession(configuration: configuration)
        let support = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
        folder = support.appendingPathComponent("Wallpapers", isDirectory: true)
        try? FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
    }

    private var isPad: Bool {
        UIDevice.current.userInterfaceIdiom == .pad
    }

    // a series title the way wallpaper sites tag it: no subtitle, no year
    static func searchName(_ title: String) -> String {
        var short = title.components(separatedBy: ":").first ?? title
        short = short.components(separatedBy: " (").first ?? short
        return short.trimmingCharacters(in: .whitespaces)
    }

    func search(_ query: String) -> Search {
        searches[query] ?? Search()
    }

    // asks for the next page, the first one the first time a search is looked at
    func load(_ query: String, more: Bool = false) async {
        var current = searches[query] ?? Search()
        guard !current.loading, current.page == 0 || (more && current.hasMore) else { return }
        current.loading = true
        current.error = nil
        searches[query] = current
        do {
            let (items, lastPage) = try await fetchPage(query: query, page: current.page + 1)
            current.items += items
            current.page += 1
            current.lastPage = lastPage
        } catch {
            current.error = NetworkMonitor.shared.isOnline ? error.localizedDescription : tr("You're offline")
        }
        current.loading = false
        searches[query] = current
    }

    func retry(_ query: String) async {
        searches[query] = nil
        await load(query)
    }

    private func fetchPage(query: String, page: Int) async throws -> ([Wallpaper], Int) {
        var components = URLComponents(string: "https://wallhaven.cc/api/v1/search")!
        var items = [
            URLQueryItem(name: "categories", value: "010"),
            URLQueryItem(name: "purity", value: "100"),
            URLQueryItem(name: "page", value: String(page)),
        ]
        // tall pictures for a phone held upright, wide ones for an ipad
        if isPad {
            items += [URLQueryItem(name: "atleast", value: "1920x1080"), URLQueryItem(name: "ratios", value: "16x9,16x10,4x3")]
        } else {
            items += [URLQueryItem(name: "atleast", value: "1080x1920"), URLQueryItem(name: "ratios", value: "portrait")]
        }
        if query.isEmpty {
            items += [URLQueryItem(name: "sorting", value: "toplist"), URLQueryItem(name: "topRange", value: "1y")]
        } else {
            items += [URLQueryItem(name: "sorting", value: "relevance"), URLQueryItem(name: "q", value: query)]
        }
        components.queryItems = items
        let (data, response) = try await session.data(from: components.url!)
        let status = (response as? HTTPURLResponse)?.statusCode ?? 0
        if status == 429 {
            throw GalleryError.busy
        }
        guard (200..<300).contains(status) else {
            throw GalleryError.http(status)
        }
        let body = try JSONDecoder().decode(WallhavenPage.self, from: data)
        // the landscape crop would only show a sliver of a tall picture, so phones get the whole one
        let pad = isPad
        let wallpapers = body.data
            .filter { $0.purity == "sfw" }
            .compactMap { item -> Wallpaper? in
                guard let full = URL(string: item.path), let thumb = URL(string: pad ? item.thumbs.large : item.thumbs.original) else { return nil }
                return Wallpaper(full: full, thumb: thumb, resolution: item.resolution.replacingOccurrences(of: "x", with: "×"))
            }
        return (wallpapers, body.meta.lastPage)
    }

    // the genre lists come from anilist in one query and are kept for a week
    func loadSeries() async {
        guard genreSeries == nil, !loadingSeries else { return }
        if let cached = DiskCache.load(CachedSeries.self, name: "anime-series"),
           Date().timeIntervalSince(cached.fetchedAt) < 7 * 24 * 60 * 60 || !NetworkMonitor.shared.isOnline {
            genreSeries = cached.sets
            return
        }
        loadingSeries = true
        defer { loadingSeries = false }
        do {
            let sets = try await fetchSeries()
            genreSeries = sets
            DiskCache.save(CachedSeries(fetchedAt: Date(), sets: sets), name: "anime-series")
        } catch {
            seriesError = NetworkMonitor.shared.isOnline ? error.localizedDescription : tr("You're offline")
        }
    }

    func retrySeries() async {
        seriesError = nil
        await loadSeries()
    }

    private func fetchSeries() async throws -> [[String]] {
        let fields = "title { romaji english }"
        var query = "query {"
        for (index, genre) in Self.genres.enumerated() {
            query += " g\(index): Page(perPage: 40) { media(type: ANIME, sort: POPULARITY_DESC, isAdult: false, genre: \"\(genre)\") { \(fields) } }"
        }
        query += " }"
        var request = URLRequest(url: URL(string: "https://graphql.anilist.co")!)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.setValue("application/json", forHTTPHeaderField: "Accept")
        request.httpBody = try JSONSerialization.data(withJSONObject: ["query": query])
        let (data, response) = try await session.data(for: request)
        let status = (response as? HTTPURLResponse)?.statusCode ?? 0
        guard (200..<300).contains(status) else {
            throw GalleryError.http(status)
        }
        guard let json = try JSONSerialization.jsonObject(with: data) as? [String: Any],
              let pages = json["data"] as? [String: Any] else {
            throw GalleryError.http(status)
        }
        // sequels look the same as the first season, only the first one stays
        let sequel = try NSRegularExpression(pattern: #"(?i)season|part \d|\b\d+(st|nd|rd|th)\b|movie|film|recap|\b(2|3|ii|iii)$"#)
        return Self.genres.indices.map { index in
            let media = ((pages["g\(index)"] as? [String: Any])?["media"] as? [[String: Any]]) ?? []
            var names: [String] = []
            for item in media {
                let title = item["title"] as? [String: Any]
                guard let full = (title?["english"] as? String) ?? (title?["romaji"] as? String) else { continue }
                let range = NSRange(full.startIndex..., in: full)
                guard sequel.firstMatch(in: full, range: range) == nil else { continue }
                let name = Self.searchName(full)
                if !names.contains(name), names.count < 24 {
                    names.append(name)
                }
            }
            return names
        }
    }

    private func file(for url: URL) -> URL {
        let digest = SHA256.hash(data: Data(url.absoluteString.utf8))
        return folder.appendingPathComponent(digest.prefix(12).map { String(format: "%02x", $0) }.joined())
    }

    func isSaved(_ url: URL) -> Bool {
        FileManager.default.fileExists(atPath: file(for: url).path)
    }

    // downloads the picture if it isn't saved yet and returns its data
    func picture(for url: URL) async throws -> Data {
        if let saved = try? Data(contentsOf: file(for: url)) {
            return saved
        }
        busy = url
        defer { busy = nil }
        var request = URLRequest(url: url)
        request.timeoutInterval = 120
        for _ in 0..<4 {
            let (data, response) = try await session.data(for: request)
            let http = response as? HTTPURLResponse
            let status = http?.statusCode ?? 0
            if status == 429 || status == 503 {
                // the server says how long to wait
                let wait = Double(http?.value(forHTTPHeaderField: "Retry-After") ?? "") ?? 10
                try await Task.sleep(for: .seconds(min(max(wait, 3), 60)))
                continue
            }
            guard (200..<300).contains(status), UIImage(data: data) != nil else {
                throw GalleryError.http(status)
            }
            try? data.write(to: file(for: url), options: .atomic)
            return data
        }
        throw GalleryError.busy
    }
}

private struct CachedSeries: Codable {
    let fetchedAt: Date
    let sets: [[String]]
}

private struct WallhavenPage: Decodable {
    struct Item: Decodable {
        struct Thumbs: Decodable {
            // large is a landscape crop, original keeps the picture's own shape
            let large: String
            let original: String
        }

        let path: String
        let purity: String
        let resolution: String
        let thumbs: Thumbs
    }

    struct Meta: Decodable {
        let lastPage: Int

        enum CodingKeys: String, CodingKey {
            case lastPage = "last_page"
        }
    }

    let data: [Item]
    let meta: Meta
}

enum GalleryError: LocalizedError {
    case busy
    case http(Int)

    var errorDescription: String? {
        switch self {
        case .busy: return tr("The server is busy, try again in a minute.")
        case .http(let status): return "HTTP \(status)"
        }
    }
}
