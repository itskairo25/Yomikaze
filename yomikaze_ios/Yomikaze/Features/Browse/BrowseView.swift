import SwiftUI

enum BrowseFeed: String, CaseIterable, Identifiable {
    case hot
    case latest
    case popular
    case newest

    var id: String { rawValue }

    var title: String {
        switch self {
        case .hot: return tr("Hot")
        case .latest: return tr("Latest")
        case .popular: return tr("Popular")
        case .newest: return tr("New")
        }
    }

    var icon: String {
        switch self {
        case .hot: return "flame.fill"
        case .latest: return "clock.fill"
        case .popular: return "star.fill"
        case .newest: return "sparkles"
        }
    }

    var rankedTitle: String {
        switch self {
        case .hot: return tr("Trending now")
        case .latest: return tr("Fresh chapters")
        case .popular: return tr("All-time top")
        case .newest: return tr("Just added")
        }
    }

    var heroCaption: String {
        switch self {
        case .hot: return tr("HOT RIGHT NOW")
        case .latest: return tr("JUST UPDATED")
        case .popular: return tr("MOST POPULAR")
        case .newest: return tr("NEW ARRIVAL")
        }
    }
}

// kept per feed so switching back is instant
struct FeedState {
    var items: [MangaSummary] = []
    var hasMore = false
    var nextPage = 1
    var isLoading = false
    var hasLoaded = false
    var error: String?
}

struct BrowseView: View {
    @Environment(AppRouter.self) private var router
    @AppStorage(SettingsKey.showAdult) private var showAdult = false

    @State private var feed: BrowseFeed = .hot
    @Namespace private var zoomNamespace
    @State private var states: [BrowseFeed: FeedState] = [:]

    private let network = NetworkMonitor.shared
    private let columns = [GridItem(.adaptive(minimum: 100, maximum: 170), spacing: 14, alignment: .top)]

    private var current: FeedState {
        states[feed] ?? FeedState()
    }

    var body: some View {
        ScrollView {
            LazyVStack(spacing: 18) {
                feedPicker

                feedContent
            }
            .padding(.horizontal, 20)
            .padding(.top, 8)
            .padding(.bottom, 24)
        }
        .background {
            AmbientBackground()
        }
        .navigationTitle(tr("Browse"))
        .navigationDestination(for: MangaSummary.self) { manga in
            MangaDetailView(manga: manga)
                .navigationTransition(.zoom(sourceID: manga.id, in: zoomNamespace))
        }
        .environment(\.zoomNamespace, zoomNamespace)
        .task(id: feed) {
            if !current.hasLoaded {
                await load(feed, reset: true)
            }
        }
        .refreshable {
            await load(feed, reset: true)
        }
        .onChange(of: showAdult) { _, _ in
            states = [:]
            Task { await load(feed, reset: true) }
        }
        .onChange(of: network.isOnline) { _, online in
            if online && current.items.isEmpty {
                Task { await load(feed, reset: true) }
            }
        }
    }

    private var feedPicker: some View {
        HStack(spacing: 8) {
            ForEach(BrowseFeed.allCases) { option in
                let selected = option == feed
                Button {
                    withAnimation(.snappy(duration: 0.25)) {
                        feed = option
                    }
                } label: {
                    HStack(spacing: 5) {
                        Image(systemName: option.icon)
                            .font(.caption)
                        Text(option.title)
                            .font(.subheadline.weight(.semibold))
                            .lineLimit(1)
                            .minimumScaleFactor(0.8)
                    }
                    .foregroundStyle(selected ? AnyShapeStyle(.white) : AnyShapeStyle(.primary))
                    .frame(maxWidth: .infinity)
                    .frame(height: 38)
                    .background(selected ? AnyShapeStyle(.tint) : AnyShapeStyle(.quaternary), in: Capsule())
                    .contentShape(Capsule())
                }
                .buttonStyle(.plain)
            }
        }
    }

    @ViewBuilder
    private var feedContent: some View {
        let state = current

        if !state.items.isEmpty {
            let ranked = Array(state.items.dropFirst().prefix(6))
            let rest = Array(state.items.dropFirst(1 + ranked.count))

            if let featured = state.items.first {
                NavigationLink(value: featured) {
                    HeroCard(manga: featured, caption: feed.heroCaption)
                }
                .buttonStyle(.plain)
            }

            // places 2 to 7 as a ranked strip
            if !ranked.isEmpty {
                VStack(alignment: .leading, spacing: 12) {
                    Text(feed.rankedTitle)
                        .font(.title3.weight(.bold))
                    ScrollView(.horizontal) {
                        HStack(alignment: .top, spacing: 16) {
                            ForEach(Array(ranked.enumerated()), id: \.element.id) { index, manga in
                                NavigationLink(value: manga) {
                                    RankedCard(manga: manga, rank: index + 2)
                                }
                                .buttonStyle(.plain)
                            }
                        }
                    }
                    .scrollIndicators(.hidden)
                    .scrollClipDisabled()
                }
                .frame(maxWidth: .infinity, alignment: .leading)
            }

            if !rest.isEmpty {
                Text(tr("More to read"))
                    .font(.title3.weight(.bold))
                    .frame(maxWidth: .infinity, alignment: .leading)
            }

            LazyVGrid(columns: columns, spacing: 18) {
                ForEach(rest) { manga in
                    NavigationLink(value: manga) {
                        BrowseCard(manga: manga)
                    }
                    .buttonStyle(.plain)
                    .onAppear {
                        if manga.id == state.items.last?.id {
                            Task { await load(feed, reset: false) }
                        }
                    }
                }
            }

            if state.isLoading {
                ProgressView()
                    .padding(.vertical, 12)
            }
        } else if !network.isOnline {
            ContentUnavailableView {
                Label(tr("You're offline"), systemImage: "wifi.slash")
            } description: {
                Text(tr("Browsing needs a connection. Your downloaded chapters are in the Library."))
            } actions: {
                Button(tr("Open Library")) {
                    router.selectedTab = .library
                }
                .buttonStyle(.glassProminent)
            }
            .padding(.top, 40)
        } else if let error = state.error {
            ErrorStateView(message: error) {
                Task { await load(feed, reset: true) }
            }
            .padding(.top, 40)
        } else if state.hasLoaded {
            ContentUnavailableView(tr("Nothing here yet"), systemImage: "tray")
                .padding(.top, 40)
        } else {
            ProgressView()
                .controlSize(.large)
                .frame(maxWidth: .infinity)
                .padding(.top, 140)
        }
    }

    private func load(_ target: BrowseFeed, reset: Bool) async {
        guard network.isOnline else {
            return
        }
        var state = states[target] ?? FeedState()
        if state.isLoading {
            return
        }
        if reset {
            state.nextPage = 1
        } else if !state.hasMore {
            return
        }
        state.isLoading = true
        state.error = nil
        states[target] = state

        let page = state.nextPage
        let includeAdult = showAdult
        let offset = (page - 1) * WeebCentral.pageSize

        do {
            let result: ResultPage
            switch target {
            case .hot:
                result = ResultPage(items: try await WeebCentral.shared.hotUpdates(), hasMore: false)
            case .latest:
                result = try await WeebCentral.shared.latestUpdates(page: page)
            case .popular:
                result = try await WeebCentral.shared.search(text: "", sort: .popularity, offset: offset, includeAdult: includeAdult)
            case .newest:
                result = try await WeebCentral.shared.search(text: "", sort: .recentlyAdded, offset: offset, includeAdult: includeAdult)
            }

            var updated = states[target] ?? FeedState()
            if reset {
                updated.items = result.items
            } else {
                let known = Set(updated.items.map { $0.id })
                updated.items.append(contentsOf: result.items.filter { !known.contains($0.id) })
            }
            updated.hasMore = result.hasMore && !result.items.isEmpty
            updated.nextPage = page + 1
            updated.hasLoaded = true
            updated.isLoading = false
            updated.error = nil
            states[target] = updated
        } catch {
            var updated = states[target] ?? FeedState()
            updated.isLoading = false
            if !Task.isCancelled {
                updated.hasLoaded = true
                updated.error = error.localizedDescription
            }
            states[target] = updated
        }
    }
}

struct RankedCard: View {
    let manga: MangaSummary
    let rank: Int

    var body: some View {
        HStack(alignment: .bottom, spacing: 2) {
            Text("\(rank)")
                .font(.system(size: 64, weight: .heavy, design: .rounded))
                .foregroundStyle(.tint)
                .opacity(0.85)
                .padding(.bottom, 38)

            VStack(alignment: .leading, spacing: 6) {
                CoverView(mangaID: manga.id, cornerRadius: 14)
                    .zoomSource(id: manga.id)
                    .frame(width: 118)
                Text(manga.title)
                    .font(.footnote.weight(.semibold))
                    .lineLimit(1)
                Text(manga.latestChapter ?? manga.metaLine)
                    .font(.caption2)
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
            }
            .frame(width: 118, alignment: .leading)
        }
        .contentShape(Rectangle())
    }
}

struct BrowseCard: View {
    let manga: MangaSummary

    private var badge: String? {
        if let chapter = manga.latestChapter, !chapter.isEmpty {
            return chapter
                .replacingOccurrences(of: "Chapter", with: "Ch.")
                .replacingOccurrences(of: "Episode", with: "Ep.")
        }
        if let type = manga.type, !type.isEmpty {
            return type
        }
        return nil
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            CoverView(mangaID: manga.id)
                .zoomSource(id: manga.id)
                .overlay(alignment: .bottomLeading) {
                    if let badge {
                        Text(badge)
                            .font(.caption2.weight(.bold))
                            .foregroundStyle(.white)
                            .lineLimit(1)
                            .padding(.horizontal, 7)
                            .padding(.vertical, 3)
                            .background(Color.black.opacity(0.62), in: Capsule())
                            .padding(6)
                    }
                }
            Text(manga.title)
                .font(.footnote.weight(.semibold))
                .lineLimit(2, reservesSpace: true)
                .multilineTextAlignment(.leading)
            if let status = manga.status, !status.isEmpty {
                Text(status)
                    .font(.caption2)
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
            } else if let date = manga.updatedAt {
                Text(date.formatted(.relative(presentation: .named)))
                    .font(.caption2)
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
            }
        }
    }
}

struct HeroCard: View {
    let manga: MangaSummary
    var caption: String = tr("HOT RIGHT NOW")

    var body: some View {
        ZStack(alignment: .bottomLeading) {
            BlurredCover(mangaID: manga.id)
                .overlay(Color.black.opacity(0.38))

            HStack(alignment: .bottom, spacing: 16) {
                CoverView(mangaID: manga.id, cornerRadius: 12)
                    .zoomSource(id: manga.id, cornerRadius: 12)
                    .frame(width: 108)

                VStack(alignment: .leading, spacing: 6) {
                    Text(caption)
                        .font(.caption2.weight(.bold))
                        .tracking(1.2)
                        .foregroundStyle(.white.opacity(0.75))
                    Text(manga.title)
                        .font(.title3.weight(.bold))
                        .foregroundStyle(.white)
                        .lineLimit(3)
                        .multilineTextAlignment(.leading)
                    let line = manga.latestChapter ?? manga.metaLine
                    if !line.isEmpty {
                        Text(line)
                            .font(.footnote)
                            .foregroundStyle(.white.opacity(0.8))
                            .lineLimit(1)
                    }
                    Label(tr("Open"), systemImage: "book.fill")
                        .font(.footnote.weight(.semibold))
                        .foregroundStyle(.white)
                        .padding(.horizontal, 14)
                        .padding(.vertical, 8)
                        .glassEffect(.regular, in: Capsule())
                        .padding(.top, 4)
                }
                Spacer(minLength: 0)
            }
            .padding(16)
        }
        .frame(height: 216)
        .clipShape(RoundedRectangle(cornerRadius: 28, style: .continuous))
        // otherwise the oversized backdrop also catches taps above and below the card
        .contentShape(RoundedRectangle(cornerRadius: 28, style: .continuous))
    }
}
