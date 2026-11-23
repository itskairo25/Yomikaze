import Foundation

// backup file shared by the ios and windows apps ("yomikaze-backup", version 1)
// dates are unix seconds, pages are zero-based
struct BackupFile: Codable {
    static let format = "yomikaze-backup"
    static let currentVersion = 1

    var format: String
    var version: Int
    var app: String
    var appVersion: String
    var exportedAt: Int64
    var manga: [BackupManga]
    var progress: [BackupProgress]
    var categories: [String]
    var recentSearches: [String]
    var settings: BackupSettings?

    init(
        app: String,
        appVersion: String,
        manga: [BackupManga],
        progress: [BackupProgress],
        categories: [String],
        recentSearches: [String],
        settings: BackupSettings?
    ) {
        self.format = BackupFile.format
        self.version = BackupFile.currentVersion
        self.app = app
        self.appVersion = appVersion
        self.exportedAt = Int64(Date().timeIntervalSince1970)
        self.manga = manga
        self.progress = progress
        self.categories = categories
        self.recentSearches = recentSearches
        self.settings = settings
    }

    // lists may be missing in older or hand-edited files
    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        format = try c.decode(String.self, forKey: .format)
        version = try c.decode(Int.self, forKey: .version)
        app = try c.decodeIfPresent(String.self, forKey: .app) ?? ""
        appVersion = try c.decodeIfPresent(String.self, forKey: .appVersion) ?? ""
        exportedAt = try c.decodeIfPresent(Int64.self, forKey: .exportedAt) ?? 0
        manga = try c.decodeIfPresent([BackupManga].self, forKey: .manga) ?? []
        progress = try c.decodeIfPresent([BackupProgress].self, forKey: .progress) ?? []
        categories = try c.decodeIfPresent([String].self, forKey: .categories) ?? []
        recentSearches = try c.decodeIfPresent([String].self, forKey: .recentSearches) ?? []
        settings = try c.decodeIfPresent(BackupSettings.self, forKey: .settings)
    }

    static func parse(_ data: Data) throws -> BackupFile {
        let file: BackupFile
        do {
            file = try JSONDecoder().decode(BackupFile.self, from: data)
        } catch {
            throw BackupError.notABackup
        }
        guard file.format == BackupFile.format else { throw BackupError.notABackup }
        guard file.version <= BackupFile.currentVersion else { throw BackupError.tooNew }
        return file
    }

    func encoded() throws -> Data {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        return try encoder.encode(self)
    }

    static func fileName(date: Date = Date()) -> String {
        let formatter = DateFormatter()
        formatter.dateFormat = "yyyy-MM-dd"
        return "Yomikaze-backup-\(formatter.string(from: date)).json"
    }

    var sourceName: String {
        switch app {
        case "ios": return "iPhone"
        case "windows": return "Windows"
        default: return app
        }
    }
}

struct BackupManga: Codable {
    var mangaId: String
    var title: String
    var author: String = ""
    var status: String = ""
    var kind: String = ""
    var inLibrary: Bool = false
    var dateAdded: Int64 = 0
    var hasHistory: Bool = false
    var lastReadAt: Int64 = 0
    var lastChapterId: String = ""
    var lastChapterName: String = ""
    var lastPage: Int = 0
    var lastPageCount: Int = 0
    var category: String = ""

    init(mangaId: String, title: String) {
        self.mangaId = mangaId
        self.title = title
    }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        mangaId = try c.decode(String.self, forKey: .mangaId)
        title = try c.decodeIfPresent(String.self, forKey: .title) ?? ""
        author = try c.decodeIfPresent(String.self, forKey: .author) ?? ""
        status = try c.decodeIfPresent(String.self, forKey: .status) ?? ""
        kind = try c.decodeIfPresent(String.self, forKey: .kind) ?? ""
        inLibrary = try c.decodeIfPresent(Bool.self, forKey: .inLibrary) ?? false
        dateAdded = try c.decodeIfPresent(Int64.self, forKey: .dateAdded) ?? 0
        hasHistory = try c.decodeIfPresent(Bool.self, forKey: .hasHistory) ?? false
        lastReadAt = try c.decodeIfPresent(Int64.self, forKey: .lastReadAt) ?? 0
        lastChapterId = try c.decodeIfPresent(String.self, forKey: .lastChapterId) ?? ""
        lastChapterName = try c.decodeIfPresent(String.self, forKey: .lastChapterName) ?? ""
        lastPage = try c.decodeIfPresent(Int.self, forKey: .lastPage) ?? 0
        lastPageCount = try c.decodeIfPresent(Int.self, forKey: .lastPageCount) ?? 0
        category = try c.decodeIfPresent(String.self, forKey: .category) ?? ""
    }
}

struct BackupProgress: Codable {
    var chapterId: String
    var mangaId: String
    var chapterName: String = ""
    var page: Int = 0
    var pageCount: Int = 0
    var completed: Bool = false
    var updatedAt: Int64 = 0

    init(chapterId: String, mangaId: String) {
        self.chapterId = chapterId
        self.mangaId = mangaId
    }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        chapterId = try c.decode(String.self, forKey: .chapterId)
        mangaId = try c.decodeIfPresent(String.self, forKey: .mangaId) ?? ""
        chapterName = try c.decodeIfPresent(String.self, forKey: .chapterName) ?? ""
        page = try c.decodeIfPresent(Int.self, forKey: .page) ?? 0
        pageCount = try c.decodeIfPresent(Int.self, forKey: .pageCount) ?? 0
        completed = try c.decodeIfPresent(Bool.self, forKey: .completed) ?? false
        updatedAt = try c.decodeIfPresent(Int64.self, forKey: .updatedAt) ?? 0
    }
}

struct BackupSettings: Codable {
    var accent: String
    // "#RRGGBB"
    var customAccent: String
    // system, light or dark
    var appearance: String
    // vertical, pagedLTR or pagedRTL
    var readerMode: String
    var tapZones: Bool
    // black, gray, sepia or white
    var readerBackground: String
    var readerDim: Double
    var readerGap: Bool
    var keepAwake: Bool
    var showAdult: Bool
    // recentlyAdded, lastRead or title
    var librarySort: String
}

enum BackupError: LocalizedError {
    case notABackup
    case tooNew
    case unreadable

    var errorDescription: String? {
        switch self {
        case .notABackup: return "This isn't a Yomikaze backup."
        case .tooNew: return "This backup was made by a newer Yomikaze. Update the app first."
        case .unreadable: return "Couldn't open the file."
        }
    }
}
