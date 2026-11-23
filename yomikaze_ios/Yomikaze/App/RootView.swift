import Observation
import SwiftData
import SwiftUI
import UIKit

enum AppTab: Hashable {
    case library
    case browse
    case search
    case history
    case settings
}

// navigation state shared by all tabs
@MainActor
@Observable
final class AppRouter {
    var selectedTab: AppTab = .library
    var librarySection: LibrarySection = .saved
    var reader: ReaderRequest?

    func open(_ chapter: Chapter, in chapters: [Chapter], mangaID: String, mangaTitle: String) {
        reader = ReaderRequest(mangaID: mangaID, mangaTitle: mangaTitle, chapters: chapters, chapter: chapter)
    }

    func resume(_ manga: SavedManga) {
        guard !manga.lastChapterID.isEmpty else { return }
        let cached = DiskCache.load([Chapter].self, name: DiskCache.chaptersName(manga.mangaID)) ?? []
        let chapter = cached.first { $0.id == manga.lastChapterID }
            ?? Chapter(id: manga.lastChapterID, name: manga.lastChapterName)
        reader = ReaderRequest(mangaID: manga.mangaID, mangaTitle: manga.title, chapters: cached, chapter: chapter)
    }

    // downloads are the only thing readable offline
    func showDownloads() {
        librarySection = .downloads
        selectedTab = .library
    }
}

enum RecentSearches {
    static func list(from raw: String) -> [String] {
        raw.split(separator: "\n").map(String.init)
    }

    static func remember(_ term: String) {
        let trimmed = term.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        let defaults = UserDefaults.standard
        var terms = list(from: defaults.string(forKey: SettingsKey.recentSearches) ?? "")
            .filter { $0.caseInsensitiveCompare(trimmed) != .orderedSame }
        terms.insert(trimmed, at: 0)
        defaults.set(terms.prefix(8).joined(separator: "\n"), forKey: SettingsKey.recentSearches)
    }
}

struct RootView: View {
    @State private var router = AppRouter()
    @State private var showSplash = true
    @State private var showOfflineToast = false
    @State private var showChangelog = false
    @State private var wasInBackground = false
    @Environment(\.scenePhase) private var scenePhase
    @Environment(\.modelContext) private var modelContext
    @Environment(\.openURL) private var openURL
    @State private var didCheckConnectionAtLaunch = false
    @Query private var recentlyRead: [SavedManga]
    @AppStorage(SettingsKey.accent) private var accentRaw = AccentChoice.yoru.rawValue
    @AppStorage(SettingsKey.customAccent) private var customAccentHex = AppTheme.defaultCustomAccent

    private let network = NetworkMonitor.shared

    init() {
        var descriptor = FetchDescriptor<SavedManga>(
            predicate: #Predicate<SavedManga> { $0.hasHistory },
            sortBy: [SortDescriptor(\SavedManga.lastReadAt, order: .reverse)]
        )
        descriptor.fetchLimit = 1
        _recentlyRead = Query(descriptor)
    }

    var body: some View {
        ZStack {
            tabs

            if showSplash {
                SplashView()
                    .transition(.opacity)
                    .zIndex(1)
            }
        }
        .overlay(alignment: .top) {
            if showOfflineToast && !showSplash {
                OfflineToast()
                    .padding(.top, 6)
                    .transition(.move(edge: .top).combined(with: .opacity))
            }
        }
        .task {
            try? await Task.sleep(nanoseconds: 1_250_000_000)
            withAnimation(.easeOut(duration: 0.35)) {
                showSplash = false
            }
            didCheckConnectionAtLaunch = true
            if Changelog.shouldPresent {
                Changelog.markSeen()
                showChangelog = true
            }
            if !network.isOnline {
                router.showDownloads()
                await flashOfflineToast()
            }
            AniListSync.shared.container = modelContext.container
            Task { await AniListSync.shared.syncIfDue() }
        }
        .onAppear {
            AppLockController.shared.lock()
            AppLockController.shared.unlock()
        }
        .onChange(of: scenePhase) { _, phase in
            if phase == .background {
                AppLockController.shared.lock()
                wasInBackground = true
            } else if phase == .active, wasInBackground {
                // the face id sheet also makes the app inactive, so only unlock after a real trip to the background
                wasInBackground = false
                AppLockController.shared.unlock()
                Task { await AniListSync.shared.syncIfDue() }
            }
        }
        .sheet(isPresented: $showChangelog) {
            ChangelogSheet()
        }
        // a series was just finished: offer to rate it on anilist
        .alert(
            tr("You finished {title}", ["title": AniListSync.shared.finished.first?.title ?? ""]),
            isPresented: Binding(
                get: { !AniListSync.shared.finished.isEmpty },
                set: { shown in
                    if !shown, !AniListSync.shared.finished.isEmpty {
                        AniListSync.shared.finished.removeFirst()
                    }
                }
            ),
            presenting: AniListSync.shared.finished.first
        ) { item in
            Button(tr("Rate on AniList")) {
                openURL(AniListSync.mangaURL(item.anilistID))
            }
            Button(tr("Not now"), role: .cancel) {}
        } message: { _ in
            Text(tr("It's marked as Completed on your AniList. Want to give it a score?"))
        }
        .onChange(of: accentKey, initial: true) { _, _ in
            applyAccentToWindows()
        }
        .onChange(of: network.isOnline) { _, online in
            guard didCheckConnectionAtLaunch else { return }
            if online {
                withAnimation(.easeOut(duration: 0.25)) {
                    showOfflineToast = false
                }
            } else {
                Task { await flashOfflineToast() }
            }
        }
    }

    private var tabs: some View {
        TabView(selection: $router.selectedTab) {
            Tab(tr("Library"), systemImage: "books.vertical.fill", value: AppTab.library) {
                NavigationStack {
                    LibraryView()
                }
            }
            Tab(tr("Browse"), systemImage: "safari.fill", value: AppTab.browse) {
                NavigationStack {
                    BrowseView()
                }
            }
            Tab(tr("Search"), systemImage: "magnifyingglass", value: AppTab.search) {
                NavigationStack {
                    SearchView()
                }
            }
            Tab(tr("History"), systemImage: "clock.fill", value: AppTab.history) {
                NavigationStack {
                    HistoryView()
                }
            }
            Tab(tr("Settings"), systemImage: "gearshape.fill", value: AppTab.settings) {
                NavigationStack {
                    SettingsView()
                }
            }
        }
        // a sidebar on ipad, the tab bar on iphone
        .tabViewStyle(.sidebarAdaptable)
        .tabBarMinimizeBehavior(.onScrollDown)
        .tabViewBottomAccessory(isEnabled: recentlyRead.first != nil) {
            ContinueReadingAccessory(manga: recentlyRead.first, router: router)
        }
        .environment(router)
        .fullScreenCover(item: $router.reader) { request in
            ReaderView(request: request)
                .environment(router)
        }
    }

    private var accentKey: String {
        accentRaw + customAccentHex
    }

    // system-drawn parts like the tab bar don't always pick up a swiftui tint change, so set it on the windows too
    private func applyAccentToWindows() {
        let color = UIColor(AppTheme.accent(raw: accentRaw, customHex: customAccentHex))
        for scene in UIApplication.shared.connectedScenes {
            guard let windowScene = scene as? UIWindowScene else { continue }
            for window in windowScene.windows {
                window.tintColor = color
            }
        }
    }

    private func flashOfflineToast() async {
        withAnimation(.spring(duration: 0.4)) {
            showOfflineToast = true
        }
        try? await Task.sleep(nanoseconds: 3_500_000_000)
        withAnimation(.easeOut(duration: 0.3)) {
            showOfflineToast = false
        }
    }
}

struct SplashView: View {
    @State private var appeared = false

    var body: some View {
        ZStack {
            AmbientBackground()

            Text("読")
                .font(.system(size: 88, weight: .bold))
                .foregroundStyle(.tint)
                .scaleEffect(appeared ? 1 : 0.84)
                .blur(radius: appeared ? 0 : 10)
                .opacity(appeared ? 1 : 0)
        }
        .onAppear {
            withAnimation(.spring(duration: 0.55, bounce: 0.25)) {
                appeared = true
            }
        }
    }
}

struct OfflineToast: View {
    var body: some View {
        Label(tr("You're offline"), systemImage: "wifi.slash")
            .font(.subheadline.weight(.semibold))
            .padding(.horizontal, 18)
            .padding(.vertical, 11)
            .glassEffect(.regular, in: Capsule())
    }
}

struct ContinueReadingAccessory: View {
    let manga: SavedManga?
    let router: AppRouter

    @Environment(\.tabViewBottomAccessoryPlacement) private var placement

    // when the tab bar is minimized the accessory only has room for one line
    private var isInline: Bool {
        placement == .inline
    }

    var body: some View {
        if let manga {
            Button {
                router.resume(manga)
            } label: {
                HStack(spacing: 10) {
                    if !isInline {
                        CoverView(mangaID: manga.mangaID, cornerRadius: 5, small: true)
                            .frame(width: 24)
                    }
                    VStack(alignment: .leading, spacing: 1) {
                        Text(manga.title)
                            .font(.footnote.weight(.semibold))
                            .lineLimit(1)
                        if !isInline {
                            Text(progressLine(manga))
                                .font(.caption2)
                                .foregroundStyle(.secondary)
                                .lineLimit(1)
                        }
                    }
                    Spacer(minLength: 8)
                    Image(systemName: "play.fill")
                        .font(isInline ? .footnote : .body)
                        .foregroundStyle(.tint)
                }
                .padding(.horizontal, 16)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
        }
    }

    private func progressLine(_ manga: SavedManga) -> String {
        if manga.lastPageCount > 0 {
            return "\(manga.lastChapterName) · \(manga.lastPage + 1)/\(manga.lastPageCount)"
        }
        return manga.lastChapterName
    }
}
