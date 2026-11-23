import SwiftData
import SwiftUI
import UIKit

struct ReaderView: View {
    let request: ReaderRequest

    @Environment(\.dismiss) private var dismiss
    @Environment(\.modelContext) private var context
    @AppStorage(SettingsKey.readerMode) private var modeRaw = ReaderMode.vertical.rawValue
    @AppStorage(SettingsKey.keepAwake) private var keepAwake = true
    @AppStorage(SettingsKey.tapZones) private var tapZones = true
    @AppStorage(SettingsKey.readerBackground) private var backgroundRaw = ReaderBackground.black.rawValue
    @AppStorage(SettingsKey.readerDim) private var dim = 0.0
    @AppStorage(SettingsKey.readerGap) private var pageGap = false
    @AppStorage(SettingsKey.readerTwoPages) private var twoPagesRaw = "auto"
    @AppStorage(SettingsKey.readerProgressBar) private var progressBar = true
    @AppStorage(SettingsKey.readerWidth) private var readerWidth = 900.0
    @State private var showChapters = false
    // the page counter that shows for a moment after each page change
    @State private var pageFlash = false
    @State private var flashTask: Task<Void, Never>?
    // page lists of the next chapter, fetched near the end so it opens at once
    @State private var nextPages: [String: [URL]] = [:]
    @State private var preparingNext: String?
    // width / height of each loaded page, for pairing pages up
    @State private var aspects: [Int: CGFloat] = [:]
    // progress is written a moment after the page settles, not on every page
    @State private var saveTask: Task<Void, Never>?
    @State private var showSettings = false
    // in screen coordinates, the same space as the tap positions
    @State private var viewFrame: CGRect = .zero

    @State private var chapters: [Chapter]
    @State private var chapter: Chapter
    @State private var pages: [URL] = []
    @State private var phase: Phase = .loading
    // page at the top (vertical) or on screen (paged)
    @State private var position: Int?
    @State private var reachedEnd = false
    @State private var showChrome = false

    enum Phase: Equatable {
        case loading
        case loaded
        case failed(String)
    }

    init(request: ReaderRequest) {
        self.request = request
        _chapters = State(initialValue: request.chapters)
        _chapter = State(initialValue: request.chapter)
    }

    private var mode: ReaderMode {
        ReaderMode(rawValue: modeRaw) ?? .vertical
    }

    private var chapterIndex: Int? {
        chapters.firstIndex { $0.id == chapter.id }
    }

    private var nextChapter: Chapter? {
        guard let index = chapterIndex, index > 0 else { return nil }
        return chapters[index - 1]
    }

    private var previousChapter: Chapter? {
        guard let index = chapterIndex, index + 1 < chapters.count else { return nil }
        return chapters[index + 1]
    }

    // two pages side by side: by itself on a wide screen (an ipad in landscape), or always or never
    private var twoPages: Bool {
        guard mode != .vertical else { return false }
        switch twoPagesRaw {
        case "on": return true
        case "off": return false
        default: return viewFrame.width >= 1000 && viewFrame.width > viewFrame.height
        }
    }

    struct Spread: Hashable {
        var first: Int
        var second: Int?
    }

    // the cover stands alone, then pages pair up. a wide page (a two-page drawing) stands alone too.
    private var spreads: [Spread] {
        let wide = { (index: Int) in (aspects[index] ?? 0.7) > 1.05 }
        var result: [Spread] = []
        var index = 0
        while index < pages.count {
            if index == 0 || wide(index) || index + 1 >= pages.count || wide(index + 1) {
                result.append(Spread(first: index))
                index += 1
            } else {
                result.append(Spread(first: index, second: index + 1))
                index += 2
            }
        }
        return result
    }

    private var currentPage: Int {
        min(max(position ?? 0, 0), max(pages.count - 1, 0))
    }

    var body: some View {
        ZStack {
            (ReaderBackground(rawValue: backgroundRaw) ?? .black).color
                .ignoresSafeArea()

            content

            // software dimming, goes darker than the lowest screen brightness
            if dim > 0 {
                Color.black
                    .opacity(dim)
                    .ignoresSafeArea()
                    .allowsHitTesting(false)
            }

            if phase == .loaded && !chromeVisible {
                pageFeedback
                    .allowsHitTesting(false)
            }

            chrome
                .opacity(chromeVisible ? 1 : 0)
                .allowsHitTesting(chromeVisible)

            if !chromeVisible {
                controlsButton
            }
        }
        .onGeometryChange(for: CGRect.self) { proxy in
            proxy.frame(in: .global)
        } action: { frame in
            viewFrame = frame
        }
        .sheet(isPresented: $showSettings) {
            ReaderSettingsSheet()
        }
        .sheet(isPresented: $showChapters) {
            ReaderChapterList(chapters: chapters, current: chapter.id, isRead: { id in
                context.progress(chapterID: id)?.completed == true
            }, pick: { target in
                if target.id != chapter.id {
                    go(to: target)
                }
            })
        }
        .onChange(of: keepAwake) { _, enabled in
            UIApplication.shared.isIdleTimerDisabled = enabled
        }
        .environment(\.colorScheme, .dark)
        .statusBarHidden(!chromeVisible)
        .persistentSystemOverlays(chromeVisible ? .automatic : .hidden)
        .task(id: chapter.id) {
            await load()
        }
        .onChange(of: position) { _, _ in
            // on the second page of a pair, show the pair
            if twoPages, let position, let spread = spreads.first(where: { $0.second == position }) {
                self.position = spread.first
                return
            }
            scheduleSave()
            prefetchUpcomingPages()
            flashPageCounter()
            prepareNextChapter()
        }
        .background {
            keyboardShortcuts
        }
        .onAppear {
            UIApplication.shared.isIdleTimerDisabled = keepAwake
        }
        .onDisappear {
            UIApplication.shared.isIdleTimerDisabled = false
            saveTask?.cancel()
            saveProgress()
            try? context.save()
        }
    }

    @ViewBuilder
    private var content: some View {
        switch phase {
        case .loading:
            ProgressView()
                .controlSize(.large)
                .tint(.white)
        case .failed(let message):
            ErrorStateView(message: message) {
                Task { await load() }
            }
        case .loaded:
            if mode == .vertical {
                verticalReader
            } else {
                pagedReader
            }
        }
    }

    private var verticalReader: some View {
        ScrollView(.vertical) {
            LazyVStack(spacing: pageGap ? 12 : 0) {
                ForEach(pages.indices, id: \.self) { index in
                    ReaderPage(url: pages[index], index: index, paged: false, onTap: handleTap)
                }
                chapterEnd
                    .id(pages.count)
            }
            // on an ipad the pages keep a readable width instead of filling a wide screen
            .frame(maxWidth: UIDevice.current.userInterfaceIdiom == .pad ? CGFloat(readerWidth) : .infinity)
            .frame(maxWidth: .infinity)
            .scrollTargetLayout()
        }
        .scrollPosition(id: $position, anchor: .top)
        .scrollIndicators(.hidden)
        .ignoresSafeArea()
        .contentShape(Rectangle())
        .onTapGesture(coordinateSpace: .global) { point in
            handleTap(point)
        }
        .id("vertical-" + chapter.id)
    }

    private var pagedReader: some View {
        ScrollView(.horizontal) {
            LazyHStack(spacing: 0) {
                if twoPages {
                    ForEach(spreads, id: \.first) { spread in
                        spreadView(spread)
                            .containerRelativeFrame([.horizontal, .vertical])
                            .environment(\.layoutDirection, .leftToRight)
                    }
                } else {
                    ForEach(pages.indices, id: \.self) { index in
                        page(index)
                            .containerRelativeFrame([.horizontal, .vertical])
                            .environment(\.layoutDirection, .leftToRight)
                    }
                }
                chapterEnd
                    .containerRelativeFrame([.horizontal, .vertical])
                    .environment(\.layoutDirection, .leftToRight)
                    .id(pages.count)
            }
            .scrollTargetLayout()
        }
        .scrollTargetBehavior(.paging)
        .scrollPosition(id: $position)
        .scrollIndicators(.hidden)
        // right-to-left paging is a mirrored scroll view
        .environment(\.layoutDirection, mode == .pagedRTL ? .rightToLeft : .leftToRight)
        .ignoresSafeArea()
        .id("paged-" + mode.rawValue + "-\(twoPages)-" + chapter.id)
    }

    private func page(_ index: Int) -> some View {
        ReaderPage(url: pages[index], index: index, paged: true, onTap: handleTap) { index, aspect in
            // only two pages need the shapes, otherwise this would redraw the reader per page
            if twoPages, aspects[index] != aspect {
                aspects[index] = aspect
            }
        }
    }

    // two pages sized to the same height and touching in the middle. in right-to-left mode
    // the first page sits on the right, like a printed manga.
    private func spreadView(_ spread: Spread) -> some View {
        GeometryReader { proxy in
            if let second = spread.second {
                let left = mode == .pagedRTL ? second : spread.first
                let right = mode == .pagedRTL ? spread.first : second
                let leftAspect = aspects[left] ?? 0.7
                let rightAspect = aspects[right] ?? 0.7
                let height = min(proxy.size.height, proxy.size.width / (leftAspect + rightAspect))
                HStack(spacing: 0) {
                    page(left)
                        .frame(width: height * leftAspect, height: height)
                    page(right)
                        .frame(width: height * rightAspect, height: height)
                }
                .frame(width: proxy.size.width, height: proxy.size.height)
            } else {
                page(spread.first)
                    .frame(width: proxy.size.width, height: proxy.size.height)
            }
        }
    }

    // arrow keys, space and escape on an ipad keyboard
    private var keyboardShortcuts: some View {
        let forward = mode == .pagedRTL ? -1 : 1
        return Group {
            Button("") { turnPage(by: forward) }
                .keyboardShortcut(.rightArrow, modifiers: [])
            Button("") { turnPage(by: -forward) }
                .keyboardShortcut(.leftArrow, modifiers: [])
            Button("") { turnPage(by: 1) }
                .keyboardShortcut(.space, modifiers: [])
            Button("") { dismiss() }
                .keyboardShortcut(.escape, modifiers: [])
            Button("") { toggleTwoPages() }
                .keyboardShortcut("t", modifiers: [])
            Button("") { showChapters = true }
                .keyboardShortcut("c", modifiers: [])
        }
        .opacity(0)
        .accessibilityHidden(true)
    }

    private func toggleTwoPages() {
        guard mode != .vertical else { return }
        twoPagesRaw = twoPages ? "off" : "on"
    }

    private var chapterEnd: some View {
        VStack(spacing: 18) {
            Image(systemName: "checkmark.circle.fill")
                .font(.system(size: 44))
                .foregroundStyle(.tint)
            Text(tr("End of {chapter}", ["chapter": chapter.name]))
                .font(.headline)
                .foregroundStyle(.white)

            if let next = nextChapter {
                Button {
                    go(to: next)
                } label: {
                    Label(tr("Next: {chapter}", ["chapter": next.name]), systemImage: "chevron.right.circle.fill")
                        .font(.headline)
                        .padding(.horizontal, 8)
                        .padding(.vertical, 4)
                }
                .buttonStyle(.glassProminent)
            } else {
                Text(tr("You're all caught up."))
                    .font(.subheadline)
                    .foregroundStyle(.white.opacity(0.6))
            }

            Button(tr("Close")) {
                dismiss()
            }
            .buttonStyle(.glass)
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, 90)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .contentShape(Rectangle())
        .onTapGesture(coordinateSpace: .global) { point in
            handleTap(point)
        }
        .onAppear {
            reachedEnd = true
            saveProgress()
        }
    }

    // controls stay up while loading or failed so there is always a way out
    private var chromeVisible: Bool {
        showChrome || phase != .loaded
    }

    // a real uikit button: a swiftui one drawn over the zoomable page (a uikit view) loses the
    // touch to the page. a tap just next to it still lands in handleTap's corner check.
    private var controlsButton: some View {
        CornerControlsButton(label: tr("Show controls")) {
            toggleChrome()
        }
        .frame(width: 46, height: 46)
        .padding(.trailing, 10)
        .padding(.bottom, 2)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .bottomTrailing)
        .transition(.opacity)
    }

    private var chrome: some View {
        VStack(spacing: 0) {
            // on an ipad the bars stay a comfortable width in the middle instead of spanning the screen
            topBar
                .frame(maxWidth: 720)
            Spacer(minLength: 0)
            if phase == .loaded {
                bottomBar
                    .frame(maxWidth: 620)
            }
        }
        .frame(maxWidth: .infinity)
        .padding(.horizontal, 16)
        .padding(.vertical, 8)
    }

    private var topBar: some View {
        GlassEffectContainer(spacing: 10) {
            HStack(spacing: 10) {
                Button {
                    dismiss()
                } label: {
                    Image(systemName: "xmark")
                        .font(.body.weight(.semibold))
                        .frame(width: 30, height: 30)
                }
                .buttonStyle(.glass)
                .buttonBorderShape(.circle)

                VStack(alignment: .leading, spacing: 1) {
                    Text(request.mangaTitle)
                        .font(.subheadline.weight(.semibold))
                        .lineLimit(1)
                    Text(chapter.name)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                }
                .padding(.horizontal, 18)
                .frame(maxWidth: .infinity, minHeight: 46, alignment: .leading)
                .glassEffect(.regular, in: Capsule())

                if mode != .vertical {
                    Button {
                        toggleTwoPages()
                    } label: {
                        Image(systemName: twoPages ? "book.pages.fill" : "rectangle.portrait")
                            .font(.body.weight(.semibold))
                            .frame(width: 30, height: 30)
                    }
                    .buttonStyle(.glass)
                    .buttonBorderShape(.circle)
                    .accessibilityLabel(tr("Two pages"))
                }

                Button {
                    showChapters = true
                } label: {
                    Image(systemName: "list.bullet")
                        .font(.body.weight(.semibold))
                        .frame(width: 30, height: 30)
                }
                .buttonStyle(.glass)
                .buttonBorderShape(.circle)
                .accessibilityLabel(tr("Chapters"))

                Button {
                    toggleChrome()
                } label: {
                    Image(systemName: "eye.slash")
                        .font(.body.weight(.semibold))
                        .frame(width: 30, height: 30)
                }
                .buttonStyle(.glass)
                .buttonBorderShape(.circle)
                .accessibilityLabel(tr("Hide controls"))

                Button {
                    showSettings = true
                } label: {
                    Image(systemName: "gearshape.fill")
                        .font(.body.weight(.semibold))
                        .frame(width: 30, height: 30)
                }
                .buttonStyle(.glass)
                .buttonBorderShape(.circle)
            }
        }
    }

    private var bottomBar: some View {
        HStack(spacing: 12) {
            Button {
                if let target = previousChapter {
                    go(to: target)
                }
            } label: {
                Image(systemName: "backward.end.fill")
                    .frame(width: 34, height: 34)
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .disabled(previousChapter == nil)
            .opacity(previousChapter == nil ? 0.35 : 1)

            VStack(spacing: 2) {
                if pages.count > 1 {
                    Slider(
                        value: Binding(
                            get: { Double(currentPage) },
                            set: { position = Int($0.rounded()) }
                        ),
                        in: 0...Double(pages.count - 1),
                        step: 1
                    )
                }
                Text("\(currentPage + 1) / \(pages.count)")
                    .font(.caption.monospacedDigit())
                    .foregroundStyle(.secondary)
            }

            Button {
                if let target = nextChapter {
                    go(to: target)
                }
            } label: {
                Image(systemName: "forward.end.fill")
                    .frame(width: 34, height: 34)
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .disabled(nextChapter == nil)
            .opacity(nextChapter == nil ? 0.35 : 1)
        }
        .padding(.horizontal, 14)
        .padding(.vertical, 10)
        .glassEffect(.regular, in: RoundedRectangle(cornerRadius: 30, style: .continuous))
    }

    // area around the controls button that counts as pressing it
    private static let controlsCorner = CGSize(width: 80, height: 70)

    // bottom-right corner shows the controls, the edges turn the page, the middle shows or hides the controls
    private func handleTap(_ point: CGPoint) {
        guard phase == .loaded, viewFrame.width > 0 else {
            return
        }
        if !chromeVisible,
           point.x > viewFrame.maxX - Self.controlsCorner.width,
           point.y > viewFrame.maxY - Self.controlsCorner.height {
            toggleChrome()
            return
        }
        let fraction = (point.x - viewFrame.minX) / viewFrame.width
        guard tapZones, fraction < 0.28 || fraction > 0.72 else {
            toggleChrome()
            return
        }
        let forward = mode == .pagedRTL ? -1 : 1
        turnPage(by: fraction < 0.28 ? -forward : forward)
    }

    // how far through the chapter, for the line along the bottom
    private var progressFraction: Double {
        guard !pages.isEmpty else { return 0 }
        if (position ?? 0) >= pages.count {
            return 1
        }
        return Double(currentPage + 1) / Double(pages.count)
    }

    // a thin progress line along the bottom and the page number for a moment after each turn
    private var pageFeedback: some View {
        VStack(spacing: 10) {
            Spacer()
            if pageFlash, (position ?? 0) < pages.count {
                Text("\(currentPage + 1) / \(pages.count)")
                    .font(.footnote.weight(.semibold).monospacedDigit())
                    .foregroundStyle(.white)
                    .padding(.horizontal, 14)
                    .padding(.vertical, 7)
                    .background(.black.opacity(0.6), in: Capsule())
                    .transition(.opacity)
            }
            if progressBar {
                GeometryReader { proxy in
                    Capsule()
                        .fill(.tint)
                        .frame(width: proxy.size.width * progressFraction, height: 3)
                        .animation(.easeOut(duration: 0.25), value: progressFraction)
                }
                .frame(height: 3)
            }
        }
        .padding(.bottom, progressBar ? 0 : 12)
        .ignoresSafeArea(.container, edges: .bottom)
    }

    private func flashPageCounter() {
        flashTask?.cancel()
        withAnimation(.easeOut(duration: 0.15)) {
            pageFlash = true
        }
        flashTask = Task {
            try? await Task.sleep(for: .seconds(1.3))
            guard !Task.isCancelled else { return }
            withAnimation(.easeIn(duration: 0.3)) {
                pageFlash = false
            }
        }
    }

    // close to the end, fetch the next chapter's page list and its first pages
    private func prepareNextChapter() {
        guard phase == .loaded, currentPage + 4 >= pages.count, let next = nextChapter else { return }
        if let urls = nextPages[next.id] {
            let first = Array(urls.prefix(2))
            Task.detached(priority: .utility) {
                for url in first {
                    _ = try? await ImageStore.shared.data(for: url)
                }
            }
            return
        }
        guard preparingNext != next.id, context.downloadedChapter(next.id) == nil, NetworkMonitor.shared.isOnline else { return }
        preparingNext = next.id
        Task {
            if let urls = try? await WeebCentral.shared.pages(chapterID: next.id), !urls.isEmpty {
                nextPages[next.id] = urls
            }
            preparingNext = nil
        }
    }

    private func turnPage(by delta: Int) {
        if twoPages {
            let list = spreads
            let current = list.firstIndex { $0.first == position || $0.second == position } ?? list.count
            let index = max(current + delta, 0)
            let target = index < list.count ? list[index].first : pages.count
            guard target != position else { return }
            withAnimation(.easeInOut(duration: 0.25)) {
                position = target
            }
            return
        }
        // pages.count is the end of chapter page
        let target = min(max((position ?? 0) + delta, 0), pages.count)
        guard target != position else { return }
        withAnimation(.easeInOut(duration: 0.25)) {
            position = target
        }
    }

    private func toggleChrome() {
        withAnimation(.easeInOut(duration: 0.2)) {
            showChrome.toggle()
        }
    }

    private func go(to target: Chapter) {
        saveProgress()
        try? context.save()
        phase = .loading
        pages = []
        position = nil
        reachedEnd = false
        chapter = target // restarts the .task(id:) above
    }

    // saving right away on every page made each page turn redraw every view that watches
    // the library, which is what made paging stutter
    private func scheduleSave() {
        saveTask?.cancel()
        saveTask = Task {
            try? await Task.sleep(for: .milliseconds(700))
            guard !Task.isCancelled else { return }
            saveProgress()
        }
    }

    private func saveProgress() {
        guard phase == .loaded, !pages.isEmpty else { return }
        let finished = reachedEnd || (position ?? 0) >= pages.count - 1
        context.recordProgress(
            mangaID: request.mangaID,
            mangaTitle: request.mangaTitle,
            chapter: chapter,
            page: currentPage,
            pageCount: pages.count,
            completed: finished
        )
    }

    private func prefetchUpcomingPages() {
        guard phase == .loaded else { return }
        let upcoming = Array(pages.dropFirst(currentPage + 1).prefix(5))
        guard !upcoming.isEmpty else { return }
        Task.detached(priority: .utility) {
            for url in upcoming {
                _ = try? await ImageStore.shared.data(for: url)
            }
        }
    }

    private func load() async {
        let target = chapter
        let mangaID = request.mangaID
        phase = .loading

        // opened from history, the chapter list may not be loaded yet
        if chapters.isEmpty {
            chapters = DiskCache.load([Chapter].self, name: DiskCache.chaptersName(mangaID)) ?? []
        }
        if chapters.isEmpty {
            Task {
                if let list = try? await WeebCentral.shared.chapters(mangaID: mangaID), !list.isEmpty {
                    chapters = list
                    DiskCache.save(list, name: DiskCache.chaptersName(mangaID))
                }
            }
        }

        do {
            let urls: [URL]
            if let download = context.downloadedChapter(target.id) {
                urls = download.pageURLs.compactMap { URL(string: $0) }
            } else if let ready = nextPages[target.id] {
                urls = ready
            } else {
                urls = try await WeebCentral.shared.pages(chapterID: target.id)
            }
            if Task.isCancelled || target.id != chapter.id {
                return
            }
            guard !urls.isEmpty else {
                throw SourceError.empty
            }

            var start = 0
            if let saved = context.progress(chapterID: target.id), !saved.completed {
                start = min(max(saved.page, 0), urls.count - 1)
            }
            reachedEnd = false
            position = start
            pages = urls
            phase = .loaded
            saveProgress()
            prefetchUpcomingPages()
        } catch {
            if Task.isCancelled {
                return
            }
            if NetworkMonitor.shared.isOnline {
                phase = .failed(error.localizedDescription)
            } else {
                phase = .failed(tr("You're offline and this chapter isn't downloaded."))
            }
        }
    }
}

// every chapter, newest first, the current one marked and read ones dimmed
struct ReaderChapterList: View {
    let chapters: [Chapter]
    let current: String
    let isRead: (String) -> Bool
    let pick: (Chapter) -> Void

    @Environment(\.dismiss) private var dismiss

    var body: some View {
        NavigationStack {
            ScrollViewReader { proxy in
                List(chapters) { item in
                    let isCurrent = item.id == current
                    let read = isRead(item.id)
                    Button {
                        pick(item)
                        dismiss()
                    } label: {
                        HStack(spacing: 12) {
                            VStack(alignment: .leading, spacing: 2) {
                                Text(item.name)
                                    .font(.body.weight(isCurrent ? .semibold : .regular))
                                    .foregroundStyle(read && !isCurrent ? Color.secondary : Color.primary)
                                if let date = item.date {
                                    Text(date, format: .dateTime.day().month(.abbreviated).year())
                                        .font(.caption)
                                        .foregroundStyle(.secondary)
                                }
                            }
                            Spacer(minLength: 8)
                            if isCurrent {
                                Image(systemName: "book.fill")
                                    .foregroundStyle(.tint)
                            } else if read {
                                Image(systemName: "checkmark")
                                    .font(.footnote.weight(.semibold))
                                    .foregroundStyle(.secondary)
                            }
                        }
                        .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                    .id(item.id)
                    .listRowBackground(isCurrent ? Color.accentColor.opacity(0.15) : nil)
                }
                .onAppear {
                    proxy.scrollTo(current, anchor: .center)
                }
            }
            .navigationTitle(tr("Chapters"))
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .confirmationAction) {
                    Button(tr("Done")) {
                        dismiss()
                    }
                }
            }
            .overlay {
                if chapters.isEmpty {
                    ProgressView()
                }
            }
        }
        .presentationDetents([.medium, .large])
    }
}

struct ReaderPage: View {
    let url: URL
    let index: Int
    let paged: Bool
    let onTap: (CGPoint) -> Void
    // tells the reader the page's width / height once it is known
    var onAspect: ((Int, CGFloat) -> Void)? = nil

    @State private var image: UIImage?
    // width / height, remembered so the layout doesn't jump when the image is released
    @State private var aspect: CGFloat = 0.7
    @State private var failed = false
    @State private var attempt = 0

    var body: some View {
        sized
            .task(id: attempt) {
                await load()
            }
            .onAppear {
                if image == nil, let cached = ImageLoader.cached(url) {
                    show(cached)
                }
            }
            .onDisappear {
                // free memory for pages that scrolled away, they come back from the cache
                image = nil
            }
    }

    @ViewBuilder
    private var sized: some View {
        if paged {
            pageContent
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        } else {
            pageContent
                .frame(maxWidth: .infinity)
                .aspectRatio(aspect, contentMode: .fit)
        }
    }

    @ViewBuilder
    private var pageContent: some View {
        if let image {
            if paged {
                ZoomableImage(image: image, onTap: onTap)
            } else {
                Image(uiImage: image)
                    .resizable()
                    .scaledToFit()
            }
        } else if failed {
            VStack(spacing: 12) {
                Image(systemName: "exclamationmark.triangle")
                    .font(.title2)
                Text(tr("Page {n} couldn't be loaded", ["n": index + 1]))
                    .font(.footnote)
                Button(tr("Retry")) {
                    failed = false
                    attempt += 1
                }
                .buttonStyle(.glass)
            }
            .foregroundStyle(.white.opacity(0.8))
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        } else {
            VStack(spacing: 10) {
                ProgressView()
                    .tint(.white)
                Text("\(index + 1)")
                    .font(.caption.monospacedDigit())
                    .foregroundStyle(.white.opacity(0.5))
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .contentShape(Rectangle())
            .onTapGesture(coordinateSpace: .global) { point in
                onTap(point)
            }
        }
    }

    private func show(_ loaded: UIImage) {
        if loaded.size.height > 0 {
            aspect = loaded.size.width / loaded.size.height
            onAspect?(index, aspect)
        }
        image = loaded
    }

    private func load() async {
        if image != nil {
            return
        }
        let loaded = await ImageLoader.image(for: url, maxWidth: 1500)
        if Task.isCancelled {
            return
        }
        if let loaded {
            show(loaded)
        } else {
            failed = true
        }
    }
}

// pinch to zoom in paged mode

struct ZoomableImage: UIViewRepresentable {
    let image: UIImage
    let onTap: (CGPoint) -> Void

    func makeCoordinator() -> Coordinator {
        Coordinator()
    }

    func makeUIView(context: Context) -> UIScrollView {
        let scrollView = UIScrollView()
        scrollView.delegate = context.coordinator
        scrollView.minimumZoomScale = 1
        scrollView.maximumZoomScale = 4
        scrollView.bouncesZoom = true
        scrollView.showsHorizontalScrollIndicator = false
        scrollView.showsVerticalScrollIndicator = false
        scrollView.contentInsetAdjustmentBehavior = .never
        scrollView.backgroundColor = .clear

        let imageView = context.coordinator.imageView
        imageView.contentMode = .scaleAspectFit
        imageView.frame = scrollView.bounds
        imageView.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        scrollView.addSubview(imageView)

        let singleTap = UITapGestureRecognizer(target: context.coordinator, action: #selector(Coordinator.handleSingleTap(_:)))
        let doubleTap = UITapGestureRecognizer(target: context.coordinator, action: #selector(Coordinator.handleDoubleTap(_:)))
        doubleTap.numberOfTapsRequired = 2
        singleTap.require(toFail: doubleTap)
        scrollView.addGestureRecognizer(singleTap)
        scrollView.addGestureRecognizer(doubleTap)

        context.coordinator.scrollView = scrollView
        return scrollView
    }

    func updateUIView(_ scrollView: UIScrollView, context: Context) {
        context.coordinator.onTap = onTap
        if context.coordinator.imageView.image !== image {
            context.coordinator.imageView.image = image
            scrollView.zoomScale = 1
        }
    }

    @MainActor
    final class Coordinator: NSObject, UIScrollViewDelegate {
        let imageView = UIImageView()
        weak var scrollView: UIScrollView?
        var onTap: ((CGPoint) -> Void)?

        func viewForZooming(in scrollView: UIScrollView) -> UIView? {
            imageView
        }

        @objc func handleSingleTap(_ recognizer: UITapGestureRecognizer) {
            // nil gives window coordinates, which are screen coordinates here
            onTap?(recognizer.location(in: nil))
        }

        @objc func handleDoubleTap(_ recognizer: UITapGestureRecognizer) {
            guard let scrollView else { return }
            if scrollView.zoomScale > 1.01 {
                scrollView.setZoomScale(1, animated: true)
            } else {
                let point = recognizer.location(in: imageView)
                let size = CGSize(width: scrollView.bounds.width / 2.5, height: scrollView.bounds.height / 2.5)
                let target = CGRect(
                    x: point.x - size.width / 2,
                    y: point.y - size.height / 2,
                    width: size.width,
                    height: size.height
                )
                scrollView.zoom(to: target, animated: true)
            }
        }
    }
}

// the small glass button in the corner that brings the reader controls up
struct CornerControlsButton: UIViewRepresentable {
    let label: String
    let action: () -> Void

    func makeCoordinator() -> Coordinator {
        Coordinator(action: action)
    }

    func makeUIView(context: Context) -> UIButton {
        var config = UIButton.Configuration.glass()
        config.image = UIImage(
            systemName: "slider.horizontal.3",
            withConfiguration: UIImage.SymbolConfiguration(pointSize: 15, weight: .semibold)
        )
        config.cornerStyle = .capsule
        let coordinator = context.coordinator
        let button = UIButton(configuration: config, primaryAction: UIAction { _ in coordinator.action() })
        button.alpha = 0.6
        button.accessibilityLabel = label
        return button
    }

    func updateUIView(_ button: UIButton, context: Context) {
        context.coordinator.action = action
        button.accessibilityLabel = label
    }

    final class Coordinator {
        var action: () -> Void

        init(action: @escaping () -> Void) {
            self.action = action
        }
    }
}
