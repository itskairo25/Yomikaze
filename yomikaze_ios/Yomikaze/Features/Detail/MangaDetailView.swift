import SwiftData
import SwiftUI
import UIKit

struct MangaDetailView: View {
    let manga: MangaSummary

    @Environment(\.modelContext) private var context
    @Environment(AppRouter.self) private var router

    @Query private var savedMatches: [SavedManga]
    @Query private var progressList: [ChapterProgress]
    @Query private var downloadList: [DownloadedChapter]

    @State private var detail: MangaDetail?
    @State private var chapters: [Chapter] = []
    @State private var isLoading = true
    @State private var hasLoaded = false
    @State private var errorMessage: String?
    @State private var summaryExpanded = false
    @State private var oldestFirst = false
    @State private var downloadedOnly = false
    @State private var pendingDownload: [Chapter] = []
    @State private var confirmDownload = false
    @State private var confirmDeleteDownloads = false

    @State private var artwork: MangaArtwork?
    @State private var bannerImage: UIImage?
    @State private var shareCard: ShareCardImage?
    @AppStorage(SettingsKey.accent) private var accentRaw = AccentChoice.yoru.rawValue
    @AppStorage(SettingsKey.customAccent) private var customAccentHex = AppTheme.defaultCustomAccent

    private let downloader = DownloadManager.shared
    private let network = NetworkMonitor.shared
    private let wallpaper = WallpaperStore.shared

    init(manga: MangaSummary) {
        self.manga = manga
        let id = manga.id
        _savedMatches = Query(filter: #Predicate<SavedManga> { $0.mangaID == id })
        _progressList = Query(filter: #Predicate<ChapterProgress> { $0.mangaID == id })
        _downloadList = Query(filter: #Predicate<DownloadedChapter> { $0.mangaID == id })
    }

    private var saved: SavedManga? { savedMatches.first }
    private var inLibrary: Bool { saved?.inLibrary == true }
    private var title: String { detail?.title ?? manga.title }

    private var progressByChapter: [String: ChapterProgress] {
        Dictionary(progressList.map { ($0.chapterID, $0) }, uniquingKeysWith: { first, _ in first })
    }

    private var downloadsByChapter: [String: DownloadedChapter] {
        Dictionary(downloadList.map { ($0.chapterID, $0) }, uniquingKeysWith: { first, _ in first })
    }

    // the chapter the main button opens: in progress, next unread, or the first
    private var resumeTarget: Chapter? {
        guard !chapters.isEmpty else { return nil }
        if let saved, let index = chapters.firstIndex(where: { $0.id == saved.lastChapterID }) {
            let finished = progressByChapter[saved.lastChapterID]?.completed == true
            // chapters are newest first, so the next one is one index lower
            if finished && index > 0 {
                return chapters[index - 1]
            }
            return chapters[index]
        }
        return chapters.last
    }

    private var resumeLabel: String {
        guard let target = resumeTarget else { return tr("Read") }
        if saved?.hasHistory == true {
            return tr("Continue · {chapter}", ["chapter": target.name])
        }
        return tr("Start · {chapter}", ["chapter": target.name])
    }

    var body: some View {
        let progress = progressByChapter
        let downloads = downloadsByChapter
        let online = network.isOnline
        let visible = visibleChapters(downloads: downloads)

        ScrollView {
            LazyVStack(alignment: .leading, spacing: 0) {
                header
                    .padding(.top, bannerImage != nil ? 96 : 8)

                if !online {
                    OfflineBanner()
                        .padding(.top, 18)
                }

                statStrip
                    .padding(.top, 22)

                progressCard(readCount: progress.values.filter { $0.completed }.count)
                    .padding(.top, 14)

                if let detail, !detail.summary.isEmpty || !detail.tags.isEmpty {
                    aboutCard(detail)
                        .padding(.top, 14)
                }

                chaptersHeader(downloadCount: downloads.count)
                    .padding(.top, 30)
                    .padding(.bottom, 6)

                if visible.isEmpty {
                    chaptersPlaceholder
                } else {
                    ForEach(visible) { chapter in
                        let item = progress[chapter.id]
                        let download = downloads[chapter.id]
                        let isRead = item?.completed == true

                        ChapterRow(
                            chapter: chapter,
                            isRead: isRead,
                            progressText: progressText(item),
                            fraction: progressFraction(item),
                            isDownloaded: download != nil,
                            unavailable: !online && download == nil,
                            open: { open(chapter) },
                            startDownload: { startDownload([chapter]) }
                        )
                        .contextMenu {
                            Button {
                                setRead(!isRead, chapter: chapter)
                            } label: {
                                Label(isRead ? tr("Mark as unread") : tr("Mark as read"), systemImage: isRead ? "circle" : "checkmark.circle")
                            }
                            if let download {
                                Button(role: .destructive) {
                                    downloader.delete(download, context: context)
                                } label: {
                                    Label(tr("Delete download"), systemImage: "trash")
                                }
                            } else {
                                Button {
                                    startDownload([chapter])
                                } label: {
                                    Label(tr("Download"), systemImage: "arrow.down.circle")
                                }
                            }
                        }
                    }
                }
            }
            // a readable width on ipad
            .frame(maxWidth: 760)
            .frame(maxWidth: .infinity)
            .padding(.horizontal, 20)
            .padding(.bottom, 40)
        }
        .background {
            backdrop
        }
        .navigationTitle("")
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                Menu {
                    Section(tr("Downloads")) {
                        Button {
                            requestDownload(unreadOnly: false)
                        } label: {
                            Label(tr("Download all chapters"), systemImage: "arrow.down.circle")
                        }
                        Button {
                            requestDownload(unreadOnly: true)
                        } label: {
                            Label(tr("Download unread chapters"), systemImage: "arrow.down.circle.dotted")
                        }
                        if !downloadList.isEmpty {
                            Button(role: .destructive) {
                                confirmDeleteDownloads = true
                            } label: {
                                Label(tr("Delete downloads"), systemImage: "trash")
                            }
                        }
                    }
                    Section(tr("Reading")) {
                        Button {
                            markAll(read: true)
                        } label: {
                            Label(tr("Mark all as read"), systemImage: "checkmark.circle")
                        }
                        Button {
                            markAll(read: false)
                        } label: {
                            Label(tr("Mark all as unread"), systemImage: "circle")
                        }
                    }
                    Section {
                        Button {
                            Task { await makeShareCard() }
                        } label: {
                            Label(tr("Share as card"), systemImage: "photo.on.rectangle")
                        }
                        ShareLink(item: WeebCentral.seriesURL(manga.id)) {
                            Label(tr("Share link"), systemImage: "square.and.arrow.up")
                        }
                        Link(destination: WeebCentral.seriesURL(manga.id)) {
                            Label(tr("Open in Safari"), systemImage: "safari")
                        }
                    }
                } label: {
                    Image(systemName: "ellipsis")
                }
            }
        }
        .confirmationDialog(
            tr("Download {n} chapters?", ["n": pendingDownload.count]),
            isPresented: $confirmDownload,
            titleVisibility: .visible
        ) {
            Button(tr("Download")) {
                startDownload(pendingDownload)
                pendingDownload = []
            }
        } message: {
            Text(tr("They are saved on this iPhone so you can read them without a connection."))
        }
        .confirmationDialog(
            tr("Delete all downloaded chapters of this series?"),
            isPresented: $confirmDeleteDownloads,
            titleVisibility: .visible
        ) {
            Button(tr("Delete downloads"), role: .destructive) {
                downloader.deleteAll(mangaID: manga.id, context: context)
            }
        }
        .sheet(item: $shareCard) { card in
            ShareCardSheet(card: card)
        }
        .task {
            if !hasLoaded {
                await load()
            }
        }
        .task(id: artwork?.banner) {
            // the header only makes room for the banner once it has loaded
            guard bannerImage == nil, let url = artwork?.banner else { return }
            if let loaded = await ImageLoader.image(for: url, maxWidth: 1400), !Task.isCancelled {
                withAnimation(.easeOut(duration: 0.35)) {
                    bannerImage = loaded
                }
            }
        }
        .refreshable {
            await load()
        }
        .onChange(of: network.isOnline) { _, isOnline in
            if isOnline && chapters.isEmpty {
                Task { await load() }
            }
        }
    }

    private func visibleChapters(downloads: [String: DownloadedChapter]) -> [Chapter] {
        var list = chapters
        if downloadedOnly {
            list = list.filter { downloads[$0.id] != nil }
        }
        if oldestFirst {
            list.reverse()
        }
        return list
    }

    private func progressText(_ item: ChapterProgress?) -> String? {
        guard let item, !item.completed, item.pageCount > 0 else { return nil }
        return tr("Page {page} of {count}", ["page": item.page + 1, "count": item.pageCount])
    }

    private func progressFraction(_ item: ChapterProgress?) -> Double? {
        guard let item, !item.completed, item.pageCount > 0 else { return nil }
        return Double(item.page + 1) / Double(item.pageCount)
    }

    private var backdrop: some View {
        ZStack(alignment: .top) {
            if wallpaper.image != nil {
                AmbientBackground()
            } else {
                Color(uiColor: .systemBackground)
                BlurredCover(mangaID: manga.id)
                    .frame(maxWidth: .infinity)
                    .frame(height: 520)
                    .opacity(0.55)
                    .overlay {
                        LinearGradient(
                            colors: [Color(uiColor: .systemBackground).opacity(0), Color(uiColor: .systemBackground)],
                            startPoint: .top,
                            endPoint: .bottom
                        )
                    }
            }

            if let bannerImage {
                Color.clear
                    .frame(maxWidth: .infinity)
                    .frame(height: 250)
                    .overlay {
                        Image(uiImage: bannerImage)
                            .resizable()
                            .scaledToFill()
                            .allowsHitTesting(false)
                    }
                    .clipped()
                    .transition(.opacity)
                    .mask {
                        LinearGradient(
                            stops: [
                                .init(color: .black, location: 0),
                                .init(color: .black, location: 0.55),
                                .init(color: .clear, location: 1),
                            ],
                            startPoint: .top,
                            endPoint: .bottom
                        )
                    }
                    .allowsHitTesting(false)
            }
        }
        .ignoresSafeArea()
    }

    private var header: some View {
        VStack(spacing: 16) {
            CoverView(mangaID: manga.id, cornerRadius: 22)
                .frame(width: 172)
                .shadow(color: .black.opacity(0.4), radius: 24, y: 14)

            VStack(spacing: 6) {
                Text(title)
                    .font(.title.weight(.bold))
                    .multilineTextAlignment(.center)
                    .lineLimit(3)
                    .minimumScaleFactor(0.7)

                if let detail, !detail.authors.isEmpty {
                    Text(detail.authors.joined(separator: ", "))
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                        .multilineTextAlignment(.center)
                        .lineLimit(2)
                }

                if detail?.official == true {
                    Label(tr("Official translation"), systemImage: "rosette")
                        .font(.caption.weight(.medium))
                        .foregroundStyle(.tint)
                        .padding(.top, 2)
                }
            }
        }
        .frame(maxWidth: .infinity)
    }

    private var statStrip: some View {
        let status = detail?.status ?? manga.status ?? ""
        let type = detail?.type ?? manga.type ?? ""
        let released = detail?.released ?? manga.year ?? ""
        let facts: [(value: String, label: String)] = [
            (chapters.isEmpty ? "-" : "\(chapters.count)", tr("Chapters")),
            (status, tr("Status")),
            (type, tr("Type")),
            (released, tr("Year")),
        ].filter { !$0.value.isEmpty }

        return HStack(spacing: 0) {
            ForEach(Array(facts.enumerated()), id: \.offset) { index, fact in
                if index > 0 {
                    Divider()
                        .frame(height: 28)
                }
                VStack(spacing: 3) {
                    Text(fact.value)
                        .font(.headline)
                        .lineLimit(1)
                        .minimumScaleFactor(0.6)
                    Text(fact.label.uppercased())
                        .font(.system(size: 10, weight: .semibold))
                        .tracking(0.8)
                        .foregroundStyle(.secondary)
                }
                .frame(maxWidth: .infinity)
                .padding(.horizontal, 4)
            }
        }
        .padding(.vertical, 14)
        .glassEffect(.regular, in: RoundedRectangle(cornerRadius: 22, style: .continuous))
    }

    private func progressCard(readCount: Int) -> some View {
        let total = max(chapters.count, readCount)
        let fraction = total > 0 ? Double(readCount) / Double(total) : 0

        return VStack(spacing: 16) {
            HStack(spacing: 16) {
                ProgressRing(fraction: fraction)
                    .frame(width: 62, height: 62)

                VStack(alignment: .leading, spacing: 4) {
                    Text(total > 0 ? tr("{read} of {total} chapters read", ["read": readCount, "total": total]) : tr("Not started"))
                        .font(.headline)
                    Text(progressSubtitle)
                        .font(.footnote)
                        .foregroundStyle(.secondary)
                        .lineLimit(2)
                }
                Spacer(minLength: 0)
            }

            GlassEffectContainer(spacing: 10) {
                HStack(spacing: 10) {
                    Button {
                        if let target = resumeTarget {
                            open(target)
                        }
                    } label: {
                        Label(resumeLabel, systemImage: "play.fill")
                            .font(.headline)
                            .lineLimit(1)
                            .frame(maxWidth: .infinity)
                            .padding(.vertical, 6)
                    }
                    .buttonStyle(.glassProminent)
                    .disabled(resumeTarget == nil)

                    Button {
                        toggleLibrary()
                    } label: {
                        Image(systemName: inLibrary ? "bookmark.fill" : "bookmark")
                            .font(.headline)
                            .frame(width: 28, height: 28)
                            .padding(.vertical, 6)
                            .contentTransition(.symbolEffect(.replace))
                    }
                    .buttonStyle(.glass)
                    .buttonBorderShape(.circle)
                    .sensoryFeedback(.selection, trigger: inLibrary)

                    Button {
                        requestDownload(unreadOnly: true)
                    } label: {
                        Image(systemName: "arrow.down.to.line")
                            .font(.headline)
                            .frame(width: 28, height: 28)
                            .padding(.vertical, 6)
                    }
                    .buttonStyle(.glass)
                    .buttonBorderShape(.circle)
                    .disabled(chapters.isEmpty)

                    if AniListSync.shared.isConnected, let anilistID = saved?.anilistID ?? detail?.anilistID {
                        let favourite = AniListSync.shared.favourites.contains(anilistID)
                        Button {
                            Task { await AniListSync.shared.toggleFavourite(anilistID) }
                        } label: {
                            Image(systemName: favourite ? "heart.fill" : "heart")
                                .font(.headline)
                                .frame(width: 28, height: 28)
                                .padding(.vertical, 6)
                                .contentTransition(.symbolEffect(.replace))
                        }
                        .buttonStyle(.glass)
                        .buttonBorderShape(.circle)
                        .accessibilityLabel(favourite ? tr("Remove from AniList favourites") : tr("Add to AniList favourites"))
                    }
                }
            }
        }
        .padding(18)
        .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 26, style: .continuous))
    }

    private var progressSubtitle: String {
        guard let saved, saved.hasHistory else {
            return tr("Tap Start to begin with the first chapter.")
        }
        if saved.lastPageCount > 0 {
            return tr("Last read: {chapter}, page {page} of {count}", ["chapter": saved.lastChapterName, "page": saved.lastPage + 1, "count": saved.lastPageCount])
        }
        return tr("Last read: {chapter}", ["chapter": saved.lastChapterName])
    }

    private func aboutCard(_ detail: MangaDetail) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            Text(tr("SYNOPSIS"))
                .font(.caption2.weight(.bold))
                .tracking(1.2)
                .foregroundStyle(.secondary)

            if !detail.summary.isEmpty {
                Text(detail.summary)
                    .font(.subheadline)
                    .lineLimit(summaryExpanded ? nil : 4)
                Button(summaryExpanded ? tr("Show less") : tr("Show more")) {
                    withAnimation(.snappy) {
                        summaryExpanded.toggle()
                    }
                }
                .font(.subheadline.weight(.semibold))
            }

            if !detail.tags.isEmpty {
                ScrollView(.horizontal) {
                    HStack(spacing: 8) {
                        ForEach(detail.tags, id: \.self) { tag in
                            Chip(text: tag)
                        }
                    }
                }
                .scrollIndicators(.hidden)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(18)
        .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 26, style: .continuous))
    }

    private func chaptersHeader(downloadCount: Int) -> some View {
        HStack(alignment: .firstTextBaseline) {
            VStack(alignment: .leading, spacing: 2) {
                Text(chapters.isEmpty ? tr("Chapters") : tr("{n} chapters", ["n": chapters.count]))
                    .font(.title3.weight(.bold))
                if downloadCount > 0 {
                    Text(tr("{n} downloaded", ["n": downloadCount]))
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
            }
            Spacer()
            Menu {
                Picker(tr("Order"), selection: $oldestFirst) {
                    Text(tr("Newest first")).tag(false)
                    Text(tr("Oldest first")).tag(true)
                }
                Toggle(tr("Downloaded only"), isOn: $downloadedOnly)
            } label: {
                Label(downloadedOnly ? tr("Downloaded") : (oldestFirst ? tr("Oldest first") : tr("Newest first")), systemImage: "line.3.horizontal.decrease")
                    .font(.subheadline)
            }
            .disabled(chapters.isEmpty)
        }
    }

    @ViewBuilder
    private var chaptersPlaceholder: some View {
        if !chapters.isEmpty && downloadedOnly {
            VStack(spacing: 10) {
                Text(tr("No downloaded chapters."))
                    .foregroundStyle(.secondary)
                Button(tr("Show all chapters")) {
                    downloadedOnly = false
                }
            }
            .frame(maxWidth: .infinity)
            .padding(.vertical, 40)
        } else if isLoading {
            ProgressView()
                .frame(maxWidth: .infinity)
                .padding(.vertical, 40)
        } else if !network.isOnline {
            Text(tr("This series hasn't been opened before, so it isn't available offline."))
                .multilineTextAlignment(.center)
                .foregroundStyle(.secondary)
                .frame(maxWidth: .infinity)
                .padding(.vertical, 40)
        } else if let errorMessage {
            ErrorStateView(message: errorMessage) {
                Task { await load() }
            }
        } else {
            Text(tr("No chapters yet."))
                .foregroundStyle(.secondary)
                .frame(maxWidth: .infinity)
                .padding(.vertical, 40)
        }
    }

    // the card is rendered off-screen in one pass and can't wait for images, so load the cover first
    private func makeShareCard() async {
        let coverURL = WeebCentral.coverURL(manga.id)
        let cover = await ImageLoader.image(for: coverURL, maxWidth: 600)
        let backdrop = await ImageLoader.blurredImage(for: WeebCentral.coverURL(manga.id, small: true))

        let view = ShareCardView(
            title: title,
            authors: detail?.authors.joined(separator: ", ") ?? "",
            chapterCount: chapters.count,
            status: detail?.status ?? manga.status ?? "",
            type: detail?.type ?? manga.type ?? "",
            year: detail?.released ?? manga.year ?? "",
            tags: detail?.tags ?? [],
            cover: cover,
            backdrop: backdrop,
            accent: AppTheme.accent(raw: accentRaw, customHex: customAccentHex)
        )
        if let image = view.rendered() {
            shareCard = ShareCardImage(image: image, title: title)
        }
    }

    private func open(_ chapter: Chapter) {
        router.open(chapter, in: chapters, mangaID: manga.id, mangaTitle: title)
    }

    private func startDownload(_ list: [Chapter]) {
        downloader.download(list, mangaID: manga.id, mangaTitle: title, context: context)
    }

    private func requestDownload(unreadOnly: Bool) {
        let progress = progressByChapter
        let downloads = downloadsByChapter
        // oldest first so chapters arrive in reading order
        let candidates = chapters.reversed().filter { chapter in
            if downloads[chapter.id] != nil || downloader.isBusy(chapter.id) {
                return false
            }
            if unreadOnly && progress[chapter.id]?.completed == true {
                return false
            }
            return true
        }
        guard !candidates.isEmpty else { return }
        pendingDownload = Array(candidates)
        confirmDownload = true
    }

    private func toggleLibrary() {
        let item = context.upsertManga(id: manga.id, title: title)
        item.inLibrary.toggle()
        if item.inLibrary {
            item.dateAdded = Date()
            applyDetail(to: item)
        } else if !item.hasHistory {
            context.delete(item)
        }
        try? context.save()
        AniListSync.changed(manga.id)
    }

    private func applyDetail(to item: SavedManga) {
        item.title = title
        if let detail {
            item.author = detail.authors.joined(separator: ", ")
            item.status = detail.status
            item.type = detail.type
        } else {
            item.status = manga.status ?? item.status
            item.type = manga.type ?? item.type
        }
    }

    private func setRead(_ read: Bool, chapter: Chapter) {
        if let existing = progressByChapter[chapter.id] {
            if read {
                existing.completed = true
            } else {
                context.delete(existing)
            }
        } else if read {
            context.insert(ChapterProgress(
                chapterID: chapter.id,
                mangaID: manga.id,
                chapterName: chapter.name,
                page: 0,
                pageCount: 0,
                completed: true
            ))
        }
        try? context.save()
        AniListSync.changed(manga.id)
    }

    // marks chapters up to the anilist progress as read, once
    private func applyAniListProgress() {
        guard let saved, let progress = saved.anilistProgress else { return }
        let existing = progressByChapter
        for chapter in chapters where existing[chapter.id]?.completed != true {
            guard let number = AniListSync.chapterNumber(chapter.name), number <= progress else { continue }
            if let item = existing[chapter.id] {
                item.completed = true
            } else {
                context.insert(ChapterProgress(
                    chapterID: chapter.id,
                    mangaID: manga.id,
                    chapterName: chapter.name,
                    page: 0,
                    pageCount: 0,
                    completed: true
                ))
            }
        }
        saved.anilistProgress = nil
        try? context.save()
    }

    private func markAll(read: Bool) {
        let existing = progressByChapter
        for chapter in chapters {
            if let item = existing[chapter.id] {
                if read {
                    item.completed = true
                } else {
                    context.delete(item)
                }
            } else if read {
                context.insert(ChapterProgress(
                    chapterID: chapter.id,
                    mangaID: manga.id,
                    chapterName: chapter.name,
                    page: 0,
                    pageCount: 0,
                    completed: true
                ))
            }
        }
        try? context.save()
        AniListSync.changed(manga.id)
    }

    private func load() async {
        let id = manga.id

        // show the cached copy right away, then refresh from the site
        if detail == nil {
            detail = DiskCache.load(MangaDetail.self, name: DiskCache.detailName(id))
        }
        if chapters.isEmpty {
            chapters = DiskCache.load([Chapter].self, name: DiskCache.chaptersName(id)) ?? []
        }
        if artwork == nil {
            artwork = DiskCache.load(MangaArtwork.self, name: DiskCache.artworkName(id))
        }

        // offline only downloaded chapters can be read, so show those first
        if !network.isOnline {
            if !downloadList.isEmpty {
                downloadedOnly = true
            }
            isLoading = false
            return
        }

        isLoading = true
        errorMessage = nil

        async let detailRequest = WeebCentral.shared.detail(mangaID: id)
        async let chaptersRequest = WeebCentral.shared.chapters(mangaID: id)

        do {
            let freshChapters = try await chaptersRequest
            if !freshChapters.isEmpty {
                chapters = freshChapters
                DiskCache.save(freshChapters, name: DiskCache.chaptersName(id))
                applyAniListProgress()
            }
        } catch {
            if !Task.isCancelled && chapters.isEmpty {
                errorMessage = error.localizedDescription
            }
        }

        if let freshDetail = try? await detailRequest {
            detail = freshDetail
            DiskCache.save(freshDetail, name: DiskCache.detailName(id))
            if let saved {
                applyDetail(to: saved)
            }
        }

        isLoading = false
        if !Task.isCancelled {
            hasLoaded = true
        }

        if artwork == nil, !Task.isCancelled {
            if let found = try? await AniList.shared.artwork(anilistID: detail?.anilistID, title: title) {
                DiskCache.save(found, name: DiskCache.artworkName(id))
                artwork = found
            }
        }
    }
}

// separate view so swiftui can skip rows whose inputs didn't change
private struct ChapterRow: View {
    let chapter: Chapter
    let isRead: Bool
    let progressText: String?
    let fraction: Double?
    let isDownloaded: Bool
    // offline and not downloaded
    let unavailable: Bool
    let open: () -> Void
    let startDownload: () -> Void

    // "Chapter 1198" becomes ("CHAPTER", "1198")
    private var parts: (kind: String, number: String)? {
        let words = chapter.name.split(separator: " ")
        guard words.count >= 2, let last = words.last, let first = last.first, first.isNumber else {
            return nil
        }
        return (words.dropLast().joined(separator: " ").uppercased(), String(last))
    }

    private var statusLine: String {
        if let progressText {
            return progressText
        }
        return isRead ? tr("Read") : tr("Not read yet")
    }

    var body: some View {
        HStack(spacing: 4) {
            Button(action: open) {
                HStack(spacing: 14) {
                    numberBadge

                    VStack(alignment: .leading, spacing: 3) {
                        if parts == nil {
                            Text(chapter.name)
                                .font(.subheadline.weight(.semibold))
                                .lineLimit(2)
                        } else if let date = chapter.date {
                            Text(date.formatted(date: .abbreviated, time: .omitted))
                                .font(.subheadline.weight(.medium))
                        }
                        Text(statusLine)
                            .font(.caption)
                            .foregroundStyle(progressText == nil ? AnyShapeStyle(.secondary) : AnyShapeStyle(.tint))
                    }

                    Spacer(minLength: 0)

                    if isRead {
                        Image(systemName: "checkmark.circle.fill")
                            .foregroundStyle(.tint)
                    }
                }
                .opacity(isRead || unavailable ? 0.5 : 1)
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .disabled(unavailable)

            DownloadControl(chapterID: chapter.id, isDownloaded: isDownloaded, start: startDownload)
        }
        .padding(.leading, 12)
        .padding(.trailing, 4)
        .padding(.vertical, 8)
        .background(Color.primary.opacity(0.06), in: RoundedRectangle(cornerRadius: 18, style: .continuous))
        .overlay(alignment: .bottom) {
            if let fraction {
                ProgressBar(fraction: fraction, height: 3)
                    .padding(.horizontal, 16)
                    .padding(.bottom, 4)
            }
        }
        .padding(.vertical, 4)
    }

    @ViewBuilder
    private var numberBadge: some View {
        if let parts {
            VStack(spacing: 0) {
                Text(parts.kind)
                    .font(.system(size: 9, weight: .bold))
                    .tracking(0.6)
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
                    .minimumScaleFactor(0.7)
                Text(parts.number)
                    .font(.system(size: 22, weight: .bold, design: .rounded).monospacedDigit())
                    .lineLimit(1)
                    .minimumScaleFactor(0.5)
            }
            .frame(width: 62)
        } else {
            Image(systemName: "book.pages")
                .font(.title3)
                .foregroundStyle(.secondary)
                .frame(width: 62)
        }
    }
}
