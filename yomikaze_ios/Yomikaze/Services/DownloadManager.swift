import Foundation
import Observation
import SwiftData

// chapters wait in a queue and only a few run at once
@MainActor
@Observable
final class DownloadManager {
    static let shared = DownloadManager()

    // chapter id to progress (0 to 1)
    private(set) var progress: [String: Double] = [:]
    private(set) var queued: Set<String> = []
    private(set) var failed: Set<String> = []

    private struct Job {
        let chapter: Chapter
        let mangaID: String
        let mangaTitle: String
        let context: ModelContext
    }

    private struct Running {
        let task: Task<Void, Never>
        let mangaID: String
    }

    @ObservationIgnored private var pending: [Job] = []
    @ObservationIgnored private var running: [String: Running] = [:]
    private let maxConcurrent = 2

    var activeCount: Int {
        progress.count + queued.count
    }

    func isBusy(_ chapterID: String) -> Bool {
        progress[chapterID] != nil || queued.contains(chapterID)
    }

    func download(_ chapter: Chapter, mangaID: String, mangaTitle: String, context: ModelContext) {
        guard !isBusy(chapter.id) else { return }
        failed.remove(chapter.id)
        queued.insert(chapter.id)
        pending.append(Job(chapter: chapter, mangaID: mangaID, mangaTitle: mangaTitle, context: context))
        pump()
    }

    func download(_ chapters: [Chapter], mangaID: String, mangaTitle: String, context: ModelContext) {
        for chapter in chapters {
            download(chapter, mangaID: mangaID, mangaTitle: mangaTitle, context: context)
        }
    }

    private func pump() {
        while running.count < maxConcurrent, !pending.isEmpty {
            let job = pending.removeFirst()
            let id = job.chapter.id
            queued.remove(id)
            progress[id] = 0
            let task = Task { [weak self] in
                await self?.run(job)
                self?.finish(id)
            }
            running[id] = Running(task: task, mangaID: job.mangaID)
        }
    }

    private func finish(_ chapterID: String) {
        running[chapterID] = nil
        progress[chapterID] = nil
        pump()
    }

    private func run(_ job: Job) async {
        let chapter = job.chapter
        do {
            let pages = try await WeebCentral.shared.pages(chapterID: chapter.id)
            var index = 0
            // a few pages at a time, quick without hammering the image host
            while index < pages.count {
                try Task.checkCancellation()
                let batch = Array(pages[index..<min(index + 4, pages.count)])
                try await withThrowingTaskGroup(of: Void.self) { group in
                    for url in batch {
                        group.addTask {
                            _ = try await ImageStore.shared.data(for: url, keep: true)
                        }
                    }
                    try await group.waitForAll()
                }
                index += batch.count
                progress[chapter.id] = Double(index) / Double(pages.count)
            }
            try Task.checkCancellation()

            // keep the cover too so the series looks right offline
            _ = try? await ImageStore.shared.data(for: WeebCentral.coverURL(job.mangaID), keep: true)
            _ = try? await ImageStore.shared.data(for: WeebCentral.coverURL(job.mangaID, small: true), keep: true)

            if job.context.downloadedChapter(chapter.id) == nil {
                job.context.insert(DownloadedChapter(
                    chapterID: chapter.id,
                    mangaID: job.mangaID,
                    mangaTitle: job.mangaTitle,
                    chapterName: chapter.name,
                    pageURLs: pages.map { $0.absoluteString }
                ))
                try? job.context.save()
            }
        } catch {
            if !(error is CancellationError) && !Task.isCancelled {
                failed.insert(chapter.id)
            }
        }
    }

    func cancel(_ chapterID: String) {
        if queued.remove(chapterID) != nil {
            pending.removeAll { $0.chapter.id == chapterID }
        }
        running[chapterID]?.task.cancel()
    }

    func cancelAll(mangaID: String) {
        let waiting = pending.filter { $0.mangaID == mangaID }.map { $0.chapter.id }
        pending.removeAll { $0.mangaID == mangaID }
        queued.subtract(waiting)
        for item in running.values where item.mangaID == mangaID {
            item.task.cancel()
        }
    }

    func delete(_ download: DownloadedChapter, context: ModelContext) {
        let urls = download.pageURLs.compactMap { URL(string: $0) }
        context.delete(download)
        try? context.save()
        Task {
            await ImageStore.shared.removeDownloads(for: urls)
        }
    }

    func deleteAll(mangaID: String, context: ModelContext) {
        cancelAll(mangaID: mangaID)
        let descriptor = FetchDescriptor<DownloadedChapter>(
            predicate: #Predicate<DownloadedChapter> { $0.mangaID == mangaID }
        )
        let items = (try? context.fetch(descriptor)) ?? []
        let urls = items.flatMap { $0.pageURLs }.compactMap { URL(string: $0) }
        for item in items {
            context.delete(item)
        }
        try? context.save()
        Task {
            await ImageStore.shared.removeDownloads(for: urls)
        }
    }

    func deleteAll(context: ModelContext) {
        pending.removeAll()
        queued.removeAll()
        for item in running.values {
            item.task.cancel()
        }
        try? context.delete(model: DownloadedChapter.self)
        try? context.save()
        Task {
            await ImageStore.shared.removeAllDownloads()
        }
    }
}
