import Foundation

// small json cache for series details and chapter lists, so they open instantly and work offline
enum DiskCache {
    private static let directory: URL = {
        let support = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
        let directory = support.appendingPathComponent("Catalog", isDirectory: true)
        try? FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        return directory
    }()

    private static func file(_ name: String) -> URL {
        directory.appendingPathComponent(name + ".json")
    }

    static func load<Value: Decodable>(_ type: Value.Type, name: String) -> Value? {
        guard let data = try? Data(contentsOf: file(name)) else { return nil }
        return try? JSONDecoder().decode(type, from: data)
    }

    static func save<Value: Encodable>(_ value: Value, name: String) {
        guard let data = try? JSONEncoder().encode(value) else { return }
        try? data.write(to: file(name), options: .atomic)
    }

    static func remove(name: String) {
        try? FileManager.default.removeItem(at: file(name))
    }

    static func detailName(_ mangaID: String) -> String { "detail-\(mangaID)" }
    static func chaptersName(_ mangaID: String) -> String { "chapters-\(mangaID)" }
    static func artworkName(_ mangaID: String) -> String { "artwork-\(mangaID)" }
}
