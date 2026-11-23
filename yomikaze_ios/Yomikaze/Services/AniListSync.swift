import Foundation
import Observation
import Security
import SwiftData

extension Notification.Name {
    // posted with the manga id when the library or reading progress changes
    static let yomikazeReadingChanged = Notification.Name("yomikazeReadingChanged")
}

// anilist sync: login, importing the manga list, and pushing the library and reading progress
// back. nothing is ever deleted on anilist.
// api rules: https://anilist.gitbook.io/anilist-apiv2-docs (currently 30 requests per minute)
//
// a full sync reads the whole list and runs at launch, every 15 minutes and on "sync now".
// changes in the app are pushed right away for just the series that changed, using what the
// last full sync saw as the anilist side.
@MainActor
@Observable
final class AniListSync {
    static let shared = AniListSync()

    // your own client from https://anilist.co/settings/developer with the redirect url
    // yomikaze://anilist, set as ANILIST_CLIENT_ID in project.yml
    static let clientID = (Bundle.main.object(forInfoDictionaryKey: "AniListClientID") as? String ?? "").trimmingCharacters(in: .whitespaces)
    static let callbackScheme = "yomikaze"

    static var isConfigured: Bool { !clientID.isEmpty }

    static var loginURL: URL {
        URL(string: "https://anilist.co/api/v2/oauth/authorize?client_id=\(clientID)&response_type=token")!
    }

    static func mangaURL(_ id: Int) -> URL {
        URL(string: "https://anilist.co/manga/\(id)")!
    }

    // tells the sync that a series changed
    nonisolated static func changed(_ mangaID: String) {
        NotificationCenter.default.post(name: .yomikazeReadingChanged, object: nil, userInfo: ["mangaID": mangaID])
    }

    struct User: Codable, Equatable {
        var id: Int
        var name: String
    }

    struct Finished: Identifiable, Equatable {
        var title: String
        var anilistID: Int
        var id: Int { anilistID }
    }

    private(set) var user: User?
    private(set) var isBusy = false
    private(set) var progressText: String?
    private(set) var errorMessage: String?
    private(set) var lastSync: Date?
    private(set) var favourites: Set<Int> = []
    // finished series waiting for the rating prompt
    var finished: [Finished] = []

    // set by the root view, syncs use its main context
    var container: ModelContainer?

    private var token: String?
    private var lastRequest: Date?
    private var lastSiteRequest: Date?
    // the anilist list as the last full sync saw it, kept up to date by pushes
    private var remote: [Int: Entry]?
    private var pending = Set<String>()
    private var pushTask: Task<Void, Never>?
    private var observer: NSObjectProtocol?

    private enum Key {
        static let user = "anilistUser"
        static let lastSync = "anilistLastSync"
        static let skip = "anilistSkip"
    }

    private enum SyncError: Error {
        case rejected
        case message(String)
    }

    private struct Entry {
        var status: String
        var progress: Int
        var updatedAt: Date
        var titles: [String]
    }

    private struct Local {
        var manga: SavedManga
        var progress: Int
        var started: Bool
    }

    var isConnected: Bool { user != nil && token != nil }

    private init() {
        if let data = UserDefaults.standard.data(forKey: Key.user),
           let saved = try? JSONDecoder().decode(User.self, from: data),
           let stored = Keychain.read() {
            user = saved
            token = stored
        }
        let stamp = UserDefaults.standard.double(forKey: Key.lastSync)
        lastSync = stamp > 0 ? Date(timeIntervalSince1970: stamp) : nil
        observer = NotificationCenter.default.addObserver(forName: .yomikazeReadingChanged, object: nil, queue: .main) { note in
            let mangaID = note.userInfo?["mangaID"] as? String
            Task { @MainActor in
                AniListSync.shared.schedulePush(mangaID)
            }
        }
    }

    func finishLogin(callback: URL) async throws {
        let fragment = callback.fragment ?? ""
        let pairs = fragment.split(separator: "&").map { $0.split(separator: "=", maxSplits: 1) }
        guard let pair = pairs.first(where: { $0.first == "access_token" }), pair.count == 2 else {
            throw URLError(.userAuthenticationRequired)
        }
        token = String(pair[1])
        remote = nil
        do {
            let data = try await api("query { Viewer { id name } }", [:])
            let viewer = data["Viewer"] as? [String: Any]
            guard let id = viewer?["id"] as? Int else { throw SyncError.rejected }
            let newUser = User(id: id, name: viewer?["name"] as? String ?? "")
            user = newUser
            Keychain.save(token ?? "")
            UserDefaults.standard.set(try? JSONEncoder().encode(newUser), forKey: Key.user)
            UserDefaults.standard.removeObject(forKey: Key.skip)
            errorMessage = nil
        } catch {
            token = nil
            throw error
        }
    }

    func disconnect() {
        pushTask?.cancel()
        Keychain.delete()
        UserDefaults.standard.removeObject(forKey: Key.user)
        token = nil
        user = nil
        remote = nil
        favourites = []
    }

    // changes go up a moment later, just for the series that changed
    private func schedulePush(_ mangaID: String?) {
        guard isConnected else { return }
        if let mangaID {
            pending.insert(mangaID)
        }
        pushTask?.cancel()
        pushTask = Task {
            try? await Task.sleep(for: .milliseconds(1500))
            guard !Task.isCancelled else { return }
            await pushPending()
        }
    }

    private func pushPending() async {
        if isBusy {
            // a running sync handles it, try again afterwards
            schedulePush(nil)
            return
        }
        let ids = pending
        pending.removeAll()
        guard !ids.isEmpty else { return }
        if remote == nil {
            await sync()
            return
        }
        await run(full: false) { context in
            let locals = self.locals(context: context, only: ids)
            for local in locals {
                _ = try await self.reconcile(local, context: context)
            }
        }
    }

    func syncIfDue() async {
        if let lastSync, Date().timeIntervalSince(lastSync) < 15 * 60, remote != nil {
            return
        }
        await sync()
    }

    func sync(full: Bool = false) async {
        guard let user else { return }
        await run(full: true) { context in
            self.remote = try await self.fetchList(userID: user.id)
            self.favourites = try await self.fetchFavourites(userID: user.id)

            var linked = Set<Int>()
            for local in self.locals(context: context, only: nil) {
                if let id = try await self.reconcile(local, context: context) {
                    linked.insert(id)
                }
            }

            // bring in what is only on anilist
            let entries = self.remote ?? [:]
            var skip = full ? Set<Int>() : Set(UserDefaults.standard.array(forKey: Key.skip) as? [Int] ?? [])
            let missing = entries.keys.filter { !linked.contains($0) && !skip.contains($0) }.sorted()
            for (index, id) in missing.enumerated() {
                self.progressText = tr("Importing {progress}...", ["progress": "\(index + 1)/\(missing.count)"])
                guard let entry = entries[id] else { continue }
                guard let detail = await self.findOnSite(id: id, titles: entry.titles) else {
                    skip.insert(id)
                    continue
                }
                let manga = context.upsertManga(id: detail.id, title: detail.title)
                if !manga.inLibrary {
                    manga.inLibrary = true
                    manga.dateAdded = Date()
                }
                manga.author = detail.authors.joined(separator: ", ")
                manga.status = detail.status
                manga.type = detail.type
                manga.anilistID = id
                if entry.progress > 0 {
                    await self.applyRemote(entry, to: manga, context: context)
                }
            }
            UserDefaults.standard.set(Array(skip), forKey: Key.skip)
            self.lastSync = Date()
            UserDefaults.standard.set(Date().timeIntervalSince1970, forKey: Key.lastSync)
        }
    }

    private func run(full: Bool, _ work: (ModelContext) async throws -> Void) async {
        guard isConnected, !isBusy, let container, !OfflineMode.isForced else { return }
        let context = container.mainContext
        isBusy = true
        if full {
            errorMessage = nil
        }
        defer {
            isBusy = false
            progressText = nil
        }
        do {
            try await work(context)
            try? context.save()
        } catch SyncError.rejected {
            disconnect()
            errorMessage = tr("The AniList login has expired. Connect again in Settings.")
        } catch SyncError.message(let message) {
            errorMessage = message
        } catch {
            errorMessage = tr("Couldn't reach AniList.")
        }
    }

    func toggleFavourite(_ id: Int) async {
        if favourites.contains(id) {
            favourites.remove(id)
        } else {
            favourites.insert(id)
        }
        let query = "mutation ($id: Int) { ToggleFavourite(mangaId: $id) { manga { pageInfo { total } } } }"
        do {
            _ = try await api(query, ["id": id])
        } catch SyncError.rejected {
            disconnect()
        } catch {}
    }

    // "Chapter 112" or "Episode 10.5" gives 112 or 10
    nonisolated static func chapterNumber(_ name: String) -> Int? {
        for word in name.split(separator: " ").reversed() {
            if let number = Double(word) {
                return Int(number.rounded(.down))
            }
        }
        return nil
    }

    // chapters read: the highest finished chapter, or the one before a started chapter
    private func locals(context: ModelContext, only: Set<String>?) -> [Local] {
        let all = (try? context.fetch(FetchDescriptor<SavedManga>())) ?? []
        let progress = (try? context.fetch(FetchDescriptor<ChapterProgress>())) ?? []
        var read: [String: Int] = [:]
        for item in progress {
            guard let number = Self.chapterNumber(item.chapterName) else { continue }
            let done = item.completed ? number : number - 1
            read[item.mangaID] = max(read[item.mangaID] ?? 0, done)
        }
        return all
            .filter { only?.contains($0.mangaID) ?? true }
            .map { Local(manga: $0, progress: max(read[$0.mangaID] ?? 0, 0), started: $0.hasHistory || read[$0.mangaID] != nil) }
    }

    // brings one series in line with anilist, both ways. returns its anilist id.
    private func reconcile(_ local: Local, context: ModelContext) async throws -> Int? {
        let manga = local.manga
        guard manga.inLibrary || local.started || manga.anilistID != nil else { return nil }
        if manga.anilistID == nil {
            manga.anilistID = await anilistID(for: manga)
        }
        guard let id = manga.anilistID else { return nil }
        let existing = remote?[id]

        // anilist is further: bring those chapters into the app
        if let existing, existing.progress > local.progress {
            await applyRemote(existing, to: manga, context: context)
            return id
        }

        let isFinished = await finished(manga, progress: local.progress)
        let status: String
        if isFinished {
            status = "COMPLETED"
        } else if let existing {
            status = existing.status == "PLANNING" && local.started ? "CURRENT" : existing.status
        } else {
            status = local.started ? "CURRENT" : "PLANNING"
        }
        let changed: Bool
        if let existing {
            changed = local.progress > existing.progress || status != existing.status
        } else {
            changed = manga.inLibrary || local.started
        }
        if changed {
            try await saveEntry(id: id, status: status, progress: local.progress)
            if isFinished && existing?.status != "COMPLETED" {
                finished.append(Finished(title: manga.title, anilistID: id))
            }
        }
        return id
    }

    // marks chapters up to the anilist progress as read and moves the history there
    private func applyRemote(_ entry: Entry, to manga: SavedManga, context: ModelContext) async {
        let chapters = await chapterList(manga.mangaID).filter { (Self.chapterNumber($0.name) ?? .max) <= entry.progress }
        guard !chapters.isEmpty else {
            // no chapter list yet, applied when the series page loads it
            manga.anilistProgress = entry.progress
            return
        }
        let mangaID = manga.mangaID
        let existing = (try? context.fetch(FetchDescriptor<ChapterProgress>(predicate: #Predicate<ChapterProgress> { $0.mangaID == mangaID }))) ?? []
        let byChapter = Dictionary(existing.map { ($0.chapterID, $0) }, uniquingKeysWith: { first, _ in first })
        for chapter in chapters {
            if let item = byChapter[chapter.id] {
                item.completed = true
            } else {
                context.insert(ChapterProgress(chapterID: chapter.id, mangaID: mangaID, chapterName: chapter.name, page: 0, pageCount: 0, completed: true))
            }
        }
        if let last = chapters.max(by: { (Self.chapterNumber($0.name) ?? 0) < (Self.chapterNumber($1.name) ?? 0) }) {
            manga.hasHistory = true
            manga.lastChapterID = last.id
            manga.lastChapterName = last.name
            manga.lastPage = 0
            manga.lastPageCount = 0
            if entry.updatedAt > manga.lastReadAt {
                manga.lastReadAt = entry.updatedAt
            }
        }
    }

    // true when the series has ended and the last chapter is read
    private func finished(_ manga: SavedManga, progress: Int) async -> Bool {
        guard progress > 0, let detail = await detail(manga.mangaID), detail.status.lowercased().contains("complet") else { return false }
        let cached = DiskCache.load([Chapter].self, name: DiskCache.chaptersName(manga.mangaID)) ?? []
        guard let last = cached.compactMap({ Self.chapterNumber($0.name) }).max() else { return false }
        return progress >= last
    }

    private func fetchList(userID: Int) async throws -> [Int: Entry] {
        let query = "query ($userId: Int) { MediaListCollection(userId: $userId, type: MANGA) { lists { entries { "
            + "mediaId status progress updatedAt media { synonyms title { romaji english native } } } } } }"
        let data = try await api(query, ["userId": userID])
        let collection = data["MediaListCollection"] as? [String: Any]
        var entries: [Int: Entry] = [:]
        for list in collection?["lists"] as? [[String: Any]] ?? [] {
            for entry in list["entries"] as? [[String: Any]] ?? [] {
                guard let id = entry["mediaId"] as? Int else { continue }
                let media = entry["media"] as? [String: Any]
                let title = media?["title"] as? [String: Any]
                var titles = ["english", "romaji", "native"].compactMap { title?[$0] as? String }
                titles += media?["synonyms"] as? [String] ?? []
                entries[id] = Entry(
                    status: entry["status"] as? String ?? "CURRENT",
                    progress: entry["progress"] as? Int ?? 0,
                    updatedAt: Date(timeIntervalSince1970: TimeInterval(entry["updatedAt"] as? Int ?? 0)),
                    titles: titles
                )
            }
        }
        return entries
    }

    private func fetchFavourites(userID: Int) async throws -> Set<Int> {
        let query = "query ($userId: Int, $page: Int) { User(id: $userId) { favourites { manga(page: $page, perPage: 50) { "
            + "nodes { id } pageInfo { hasNextPage } } } } }"
        var ids = Set<Int>()
        for page in 1...10 {
            let data = try await api(query, ["userId": userID, "page": page])
            let manga = ((data["User"] as? [String: Any])?["favourites"] as? [String: Any])?["manga"] as? [String: Any]
            for node in manga?["nodes"] as? [[String: Any]] ?? [] {
                if let id = node["id"] as? Int {
                    ids.insert(id)
                }
            }
            if (manga?["pageInfo"] as? [String: Any])?["hasNextPage"] as? Bool != true {
                break
            }
        }
        return ids
    }

    private func saveEntry(id: Int, status: String, progress: Int) async throws {
        let query = "mutation ($mediaId: Int, $status: MediaListStatus, $progress: Int) { "
            + "SaveMediaListEntry(mediaId: $mediaId, status: $status, progress: $progress) { id } }"
        _ = try await api(query, ["mediaId": id, "status": status, "progress": progress])
        let titles = remote?[id]?.titles ?? []
        remote?[id] = Entry(status: status, progress: progress, updatedAt: Date(), titles: titles)
    }

    // series details come from the disk cache when possible, otherwise from weebcentral
    private func detail(_ mangaID: String) async -> MangaDetail? {
        if let cached = DiskCache.load(MangaDetail.self, name: DiskCache.detailName(mangaID)) {
            return cached
        }
        await waitForSite()
        guard let fresh = try? await WeebCentral.shared.detail(mangaID: mangaID) else { return nil }
        DiskCache.save(fresh, name: DiskCache.detailName(mangaID))
        return fresh
    }

    private func chapterList(_ mangaID: String) async -> [Chapter] {
        if let cached = DiskCache.load([Chapter].self, name: DiskCache.chaptersName(mangaID)), !cached.isEmpty {
            return cached
        }
        await waitForSite()
        guard let fresh = try? await WeebCentral.shared.chapters(mangaID: mangaID), !fresh.isEmpty else { return [] }
        DiskCache.save(fresh, name: DiskCache.chaptersName(mangaID))
        return fresh
    }

    private func anilistID(for manga: SavedManga) async -> Int? {
        if let id = await detail(manga.mangaID)?.anilistID {
            return id
        }
        let query = "query ($search: String) { Media(search: $search, type: MANGA) { id } }"
        let data = try? await api(query, ["search": manga.title])
        return (data?["Media"] as? [String: Any])?["id"] as? Int
    }

    // finds the weebcentral series for an anilist entry
    private func findOnSite(id: Int, titles: [String]) async -> MangaDetail? {
        let wanted = Set(titles.map(Self.normalize))
        for title in titles.prefix(2) {
            await waitForSite()
            guard let page = try? await WeebCentral.shared.search(text: title, sort: .bestMatch, includeAdult: true) else { continue }
            for card in page.items.prefix(3) {
                guard let found = await detail(card.id) else { continue }
                if found.anilistID == id || (found.anilistID == nil && wanted.contains(Self.normalize(found.title))) {
                    return found
                }
            }
        }
        return nil
    }

    private static func normalize(_ title: String) -> String {
        String(title.lowercased().filter { $0.isLetter || $0.isNumber })
    }

    private func waitForSite() async {
        if let last = lastSiteRequest {
            let gap = 1.2 - Date().timeIntervalSince(last)
            if gap > 0 {
                try? await Task.sleep(for: .seconds(gap))
            }
        }
        lastSiteRequest = Date()
    }

    // stays under the 30 requests per minute limit and waits out 429 answers
    private func api(_ query: String, _ variables: [String: Any]) async throws -> [String: Any] {
        for _ in 0..<3 {
            if let last = lastRequest {
                let gap = 2.1 - Date().timeIntervalSince(last)
                if gap > 0 {
                    try? await Task.sleep(for: .seconds(gap))
                }
            }
            lastRequest = Date()

            var request = URLRequest(url: URL(string: "https://graphql.anilist.co")!, timeoutInterval: 30)
            request.httpMethod = "POST"
            request.setValue("application/json", forHTTPHeaderField: "Content-Type")
            request.setValue("application/json", forHTTPHeaderField: "Accept")
            if let token {
                request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
            }
            request.httpBody = try JSONSerialization.data(withJSONObject: ["query": query, "variables": variables])

            let (data, response) = try await URLSession.shared.data(for: request)
            let http = response as? HTTPURLResponse
            let status = http?.statusCode ?? 0
            if status == 429 {
                let wait = Double(http?.value(forHTTPHeaderField: "Retry-After") ?? "") ?? 60
                try? await Task.sleep(for: .seconds(min(wait, 90)))
                continue
            }
            let text = String(data: data, encoding: .utf8) ?? ""
            if status == 401 || (status == 400 && text.contains("Invalid token")) {
                throw SyncError.rejected
            }
            let json = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any] ?? [:]
            guard (200..<300).contains(status) else {
                let message = ((json["errors"] as? [[String: Any]])?.first?["message"] as? String) ?? "HTTP \(status)"
                throw SyncError.message("AniList: \(message)")
            }
            return json["data"] as? [String: Any] ?? [:]
        }
        throw SyncError.message(tr("AniList is busy, try again in a minute."))
    }
}

// the anilist token lives in the keychain
private enum Keychain {
    private static let service = "dev.kairo.yomikaze.anilist"

    static func save(_ token: String) {
        delete()
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccessible as String: kSecAttrAccessibleAfterFirstUnlock,
            kSecValueData as String: Data(token.utf8),
        ]
        SecItemAdd(query as CFDictionary, nil)
    }

    static func read() -> String? {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecReturnData as String: true,
            kSecMatchLimit as String: kSecMatchLimitOne,
        ]
        var result: AnyObject?
        guard SecItemCopyMatching(query as CFDictionary, &result) == errSecSuccess, let data = result as? Data else { return nil }
        return String(data: data, encoding: .utf8)
    }

    static func delete() {
        let query: [String: Any] = [kSecClass as String: kSecClassGenericPassword, kSecAttrService as String: service]
        SecItemDelete(query as CFDictionary)
    }
}
