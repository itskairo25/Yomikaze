import Foundation

// smoke test for the weebcentral client, runs on ci and is not part of the app
// compiled together with the foundation-only app sources, see .github/workflows/build.yml

@main
struct SmokeTest {
    static func main() async {
        var failures = 0

        func check(_ name: String, _ passed: Bool, _ info: String) {
            print("\(passed ? "PASS" : "FAIL")  \(name): \(info)")
            if !passed {
                failures += 1
            }
        }

        // a backup written by the windows app must open here
        do {
            let data = try Data(contentsOf: URL(fileURLWithPath: "Tools/SmokeTest/windows-backup-sample.json"))
            let file = try BackupFile.parse(data)
            let manga = file.manga.first
            check("windows backup", file.app == "windows" && manga?.lastPage == 13 && manga?.lastPageCount == 57
                  && manga?.inLibrary == true && manga?.category == "Reading" && file.progress.first?.page == 13
                  && file.settings?.readerMode == "pagedRTL",
                  "\(file.manga.count) series, page \(manga?.lastPage ?? -1)/\(manga?.lastPageCount ?? -1), mode \(file.settings?.readerMode ?? "-")")
            let again = try BackupFile.parse(try file.encoded())
            check("backup round trip", again.manga.first?.lastPage == 13 && again.progress.count == file.progress.count, "re-encoded and parsed")
        } catch {
            check("windows backup", false, "\(error)")
        }

        let source = WeebCentral.shared

        do {
            let popular = try await source.search(text: "", sort: .popularity)
            check("popular", popular.items.count >= 20 && popular.hasMore,
                  "\(popular.items.count) items, hasMore=\(popular.hasMore), first=\(popular.items.first?.title ?? "-") [\(popular.items.first?.metaLine ?? "")]")

            let second = try await source.search(text: "", sort: .popularity, offset: WeebCentral.pageSize)
            let overlap = Set(popular.items.map { $0.id }).intersection(second.items.map { $0.id }).count
            check("popular page 2", second.items.count >= 20, "\(second.items.count) items, \(overlap) repeated from page 1")

            let text = try await source.search(text: "naruto", sort: .bestMatch)
            check("text search", text.items.contains { $0.title.lowercased().contains("naruto") },
                  "\(text.items.count) items, first=\(text.items.first?.title ?? "-")")

            let tagged = try await source.search(text: "", sort: .popularity, tag: "Isekai")
            check("genre search", tagged.items.count >= 10, "\(tagged.items.count) items")

            let newest = try await source.search(text: "", sort: .recentlyAdded)
            check("recently added", newest.items.count >= 10, "\(newest.items.count) items")

            let latest = try await source.latestUpdates()
            check("latest", latest.items.count >= 10 && latest.items.allSatisfy { $0.latestChapter != nil },
                  "\(latest.items.count) items, hasMore=\(latest.hasMore), first=\(latest.items.first?.title ?? "-") / \(latest.items.first?.latestChapter ?? "-")")

            let hot = try await source.hotUpdates()
            check("hot", hot.count >= 10, "\(hot.count) items")

            if let first = popular.items.first {
                let detail = try await source.detail(mangaID: first.id)
                check("detail", !detail.title.isEmpty && !detail.tags.isEmpty && !detail.summary.isEmpty,
                      "\(detail.title) by \(detail.authors.joined(separator: ", ")), \(detail.tags.count) tags, status=\(detail.status)")

                let art = try await AniList.shared.artwork(anilistID: detail.anilistID, title: detail.title)
                check("artwork", detail.anilistID != nil && art.banner != nil,
                      "anilistID=\(detail.anilistID.map(String.init) ?? "-"), banner=\(art.banner?.lastPathComponent ?? "-")")

                let started = Date()
                let chapters = try await source.chapters(mangaID: first.id)
                let elapsed = Date().timeIntervalSince(started)
                check("chapters", !chapters.isEmpty && chapters.allSatisfy { $0.date != nil },
                      "\(chapters.count) chapters in \(String(format: "%.2f", elapsed))s (download + parse), newest=\(chapters.first?.name ?? "-")")

                if let oldest = chapters.last {
                    let pages = try await source.pages(chapterID: oldest.id)
                    check("pages", pages.count >= 3, "\(pages.count) pages, first=\(pages.first?.absoluteString ?? "-")")
                }
            }
        } catch {
            print("ERROR  \(error)")
            failures += 1
        }

        print(failures == 0 ? "ALL CHECKS PASSED" : "\(failures) CHECK(S) FAILED")
        exit(failures == 0 ? 0 : 1)
    }
}
