import Foundation

struct MangaArtwork: Codable, Hashable, Sendable {
    var banner: URL?
}

// weebcentral links the exact anilist entry, the title search is only a fallback
struct AniList: Sendable {
    static let shared = AniList()

    private let session: URLSession

    init() {
        let configuration = URLSessionConfiguration.default
        configuration.timeoutIntervalForRequest = 20
        session = URLSession(configuration: configuration)
    }

    func artwork(anilistID: Int?, title: String) async throws -> MangaArtwork {
        if OfflineMode.isForced {
            throw URLError(.notConnectedToInternet)
        }

        let fields = "bannerImage"
        let body: [String: Any]
        if let anilistID {
            body = [
                "query": "query ($id: Int) { Media(id: $id, type: MANGA) { \(fields) } }",
                "variables": ["id": anilistID],
            ]
        } else {
            body = [
                "query": "query ($search: String) { Media(search: $search, type: MANGA) { \(fields) } }",
                "variables": ["search": title],
            ]
        }

        var request = URLRequest(url: URL(string: "https://graphql.anilist.co")!)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.setValue("application/json", forHTTPHeaderField: "Accept")
        request.httpBody = try JSONSerialization.data(withJSONObject: body)

        let (data, response) = try await session.data(for: request)
        guard let http = response as? HTTPURLResponse else {
            throw SourceError.invalidResponse
        }
        // 404 means anilist doesn't know the series
        if http.statusCode == 404 {
            return MangaArtwork(banner: nil)
        }
        guard (200..<300).contains(http.statusCode) else {
            throw SourceError.badStatus(http.statusCode)
        }

        let decoded = try JSONDecoder().decode(Response.self, from: data)
        let media = decoded.data?.Media
        return MangaArtwork(banner: media?.bannerImage.flatMap { URL(string: $0) })
    }

    private struct Response: Decodable {
        struct Payload: Decodable {
            let Media: Media?
        }

        struct Media: Decodable {
            let bannerImage: String?
        }

        let data: Payload?
    }
}
