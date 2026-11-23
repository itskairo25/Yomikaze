import SwiftUI

struct Genre: Identifiable, Hashable {
    let name: String
    let icon: String

    var id: String { name }

    static let all: [Genre] = [
        Genre(name: "Action", icon: "flame.fill"),
        Genre(name: "Adventure", icon: "map.fill"),
        Genre(name: "Comedy", icon: "face.smiling.inverse"),
        Genre(name: "Drama", icon: "theatermasks.fill"),
        Genre(name: "Fantasy", icon: "wand.and.stars"),
        Genre(name: "Historical", icon: "building.columns.fill"),
        Genre(name: "Horror", icon: "moon.haze.fill"),
        Genre(name: "Isekai", icon: "globe.asia.australia.fill"),
        Genre(name: "Martial Arts", icon: "figure.martial.arts"),
        Genre(name: "Mecha", icon: "gearshape.2.fill"),
        Genre(name: "Mystery", icon: "magnifyingglass"),
        Genre(name: "Psychological", icon: "brain.head.profile"),
        Genre(name: "Romance", icon: "heart.fill"),
        Genre(name: "School Life", icon: "graduationcap.fill"),
        Genre(name: "Sci-fi", icon: "atom"),
        Genre(name: "Seinen", icon: "person.fill"),
        Genre(name: "Shoujo", icon: "sparkles"),
        Genre(name: "Shounen", icon: "bolt.fill"),
        Genre(name: "Slice of Life", icon: "cup.and.saucer.fill"),
        Genre(name: "Sports", icon: "figure.run"),
        Genre(name: "Supernatural", icon: "eye.fill"),
        Genre(name: "Tragedy", icon: "cloud.rain.fill"),
    ]
}

struct SearchView: View {
    @Environment(AppRouter.self) private var router
    @AppStorage(SettingsKey.showAdult) private var showAdult = false
    @AppStorage(SettingsKey.recentSearches) private var recentRaw = ""

    @State private var query = ""
    @Namespace private var zoomNamespace
    @FocusState private var fieldFocused: Bool
    @State private var tag: String?
    @State private var sort: SearchSort = .bestMatch
    @State private var results: [MangaSummary] = []
    @State private var hasMore = false
    @State private var isSearching = false
    @State private var isLoadingMore = false
    @State private var errorMessage: String?

    private let network = NetworkMonitor.shared

    private var trimmedQuery: String {
        query.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    private var isActive: Bool {
        !trimmedQuery.isEmpty || tag != nil
    }

    private var effectiveSort: SearchSort {
        (trimmedQuery.isEmpty && sort == .bestMatch) ? .popularity : sort
    }

    // changing any part of this restarts the search
    private var searchKey: String {
        "\(trimmedQuery)|\(tag ?? "")|\(sort.rawValue)|\(showAdult)|\(network.isOnline)"
    }

    private var recentSearches: [String] {
        RecentSearches.list(from: recentRaw)
    }

    var body: some View {
        VStack(spacing: 0) {
            searchField
                .padding(.horizontal, 20)
                .padding(.top, 4)
                .padding(.bottom, 10)

            if isActive {
                resultsList
            } else {
                discover
            }
        }
        .background {
            AmbientBackground()
        }
        .navigationTitle(tr(tag ?? "Search"))
        .toolbar {
            if isActive {
                ToolbarItem(placement: .topBarTrailing) {
                    Menu {
                        Picker(tr("Sort by"), selection: $sort) {
                            ForEach(SearchSort.allCases) { option in
                                Text(tr(option.rawValue)).tag(option)
                            }
                        }
                    } label: {
                        Image(systemName: "arrow.up.arrow.down")
                    }
                }
            }
            if tag != nil {
                ToolbarItem(placement: .topBarLeading) {
                    Button {
                        tag = nil
                    } label: {
                        Image(systemName: "xmark")
                    }
                }
            }
        }
        .navigationDestination(for: MangaSummary.self) { manga in
            MangaDetailView(manga: manga)
                .navigationTransition(.zoom(sourceID: manga.id, in: zoomNamespace))
        }
        .environment(\.zoomNamespace, zoomNamespace)
        .task(id: searchKey) {
            await runSearch()
        }
    }

    private var searchField: some View {
        HStack(spacing: 10) {
            Image(systemName: "magnifyingglass")
                .foregroundStyle(.secondary)
            TextField(tr("Titles, authors..."), text: $query)
                .focused($fieldFocused)
                .submitLabel(.search)
                .autocorrectionDisabled()
                .textInputAutocapitalization(.never)
                .onSubmit {
                    RecentSearches.remember(query)
                }
            if !query.isEmpty {
                Button {
                    query = ""
                    fieldFocused = true
                } label: {
                    Image(systemName: "xmark.circle.fill")
                        .foregroundStyle(.secondary)
                }
                .buttonStyle(.plain)
            }
        }
        .padding(.horizontal, 16)
        .frame(height: 48)
        .glassEffect(.regular.interactive(), in: Capsule())
    }

    @ViewBuilder
    private var resultsList: some View {
        if !network.isOnline {
            ContentUnavailableView {
                Label(tr("You're offline"), systemImage: "wifi.slash")
            } description: {
                Text(tr("Search needs a connection."))
            }
            .frame(maxHeight: .infinity)
        } else if isSearching && results.isEmpty {
            ProgressView()
                .controlSize(.large)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        } else if let errorMessage, results.isEmpty {
            ErrorStateView(message: errorMessage) {
                Task { await runSearch() }
            }
            .frame(maxHeight: .infinity)
        } else if results.isEmpty {
            ContentUnavailableView.search(text: trimmedQuery.isEmpty ? (tag ?? "") : trimmedQuery)
                .frame(maxHeight: .infinity)
        } else {
            List {
                ForEach(results) { manga in
                    NavigationLink(value: manga) {
                        MangaRow(manga: manga)
                    }
                    .listRowBackground(Color.clear)
                    .onAppear {
                        if manga.id == results.last?.id {
                            Task { await loadMore() }
                        }
                    }
                }
                if isLoadingMore {
                    HStack {
                        Spacer()
                        ProgressView()
                        Spacer()
                    }
                    .listRowSeparator(.hidden)
                    .listRowBackground(Color.clear)
                }
            }
            .listStyle(.plain)
            .scrollContentBackground(.hidden)
            .scrollDismissesKeyboard(.immediately)
        }
    }

    private var discover: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 28) {
                if !network.isOnline {
                    OfflineBanner(text: tr("You're offline. Search needs a connection."))
                }

                if !recentSearches.isEmpty {
                    VStack(alignment: .leading, spacing: 12) {
                        HStack {
                            Text(tr("Recent"))
                                .font(.title3.weight(.bold))
                            Spacer()
                            Button(tr("Clear")) {
                                recentRaw = ""
                            }
                            .font(.subheadline)
                        }
                        ScrollView(.horizontal) {
                            HStack(spacing: 8) {
                                ForEach(recentSearches, id: \.self) { term in
                                    Button {
                                        query = term
                                    } label: {
                                        Chip(text: term, systemImage: "clock.arrow.circlepath")
                                    }
                                    .buttonStyle(.plain)
                                }
                            }
                        }
                        .scrollIndicators(.hidden)
                    }
                }

                VStack(alignment: .leading, spacing: 12) {
                    Text(tr("Genres"))
                        .font(.title3.weight(.bold))
                    LazyVGrid(columns: [GridItem(.flexible(), spacing: 12), GridItem(.flexible(), spacing: 12)], spacing: 12) {
                        ForEach(Array(Genre.all.enumerated()), id: \.element.id) { index, genre in
                            Button {
                                tag = genre.name
                            } label: {
                                GenreTile(genre: genre, hue: Double(index) / Double(Genre.all.count))
                            }
                            .buttonStyle(.plain)
                        }
                    }
                }
            }
            .padding(.horizontal, 20)
            .padding(.top, 8)
            .padding(.bottom, 24)
        }
        .scrollDismissesKeyboard(.immediately)
    }

    private func runSearch() async {
        guard isActive, network.isOnline else {
            results = []
            hasMore = false
            errorMessage = nil
            isSearching = false
            return
        }
        let text = trimmedQuery
        let selectedTag = tag
        let selectedSort = effectiveSort
        let includeAdult = showAdult

        isSearching = true
        // debounce typing
        try? await Task.sleep(nanoseconds: 350_000_000)
        if Task.isCancelled { return }

        do {
            let page = try await WeebCentral.shared.search(
                text: text,
                sort: selectedSort,
                tag: selectedTag,
                includeAdult: includeAdult
            )
            if Task.isCancelled { return }
            results = page.items
            hasMore = page.hasMore
            errorMessage = nil
        } catch {
            if Task.isCancelled { return }
            results = []
            hasMore = false
            errorMessage = error.localizedDescription
        }
        isSearching = false
    }

    private func loadMore() async {
        guard hasMore, !isLoadingMore, !isSearching else { return }
        let key = searchKey
        let offset = results.count
        let text = trimmedQuery
        let selectedTag = tag
        let selectedSort = effectiveSort
        let includeAdult = showAdult

        isLoadingMore = true
        defer { isLoadingMore = false }

        guard let page = try? await WeebCentral.shared.search(
            text: text,
            sort: selectedSort,
            tag: selectedTag,
            offset: offset,
            includeAdult: includeAdult
        ) else { return }
        // the user changed the search in the meantime
        guard key == searchKey else { return }

        let known = Set(results.map { $0.id })
        results.append(contentsOf: page.items.filter { !known.contains($0.id) })
        hasMore = page.hasMore && !page.items.isEmpty
    }
}

struct GenreTile: View {
    let genre: Genre
    let hue: Double

    var body: some View {
        let secondHue = hue + 0.06 > 1 ? hue + 0.06 - 1 : hue + 0.06
        ZStack(alignment: .bottomLeading) {
            LinearGradient(
                colors: [
                    Color(hue: hue, saturation: 0.62, brightness: 0.78),
                    Color(hue: secondHue, saturation: 0.72, brightness: 0.52),
                ],
                startPoint: .topLeading,
                endPoint: .bottomTrailing
            )

            Image(systemName: genre.icon)
                .font(.system(size: 46, weight: .semibold))
                .foregroundStyle(.white.opacity(0.28))
                .rotationEffect(.degrees(-12))
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topTrailing)
                .offset(x: 6, y: -4)

            HStack(spacing: 6) {
                Image(systemName: genre.icon)
                    .font(.subheadline.weight(.semibold))
                Text(tr(genre.name))
                    .font(.headline)
                    .lineLimit(1)
                    .minimumScaleFactor(0.8)
            }
            .foregroundStyle(.white)
            .padding(12)
        }
        .frame(height: 84)
        .clipShape(RoundedRectangle(cornerRadius: 18, style: .continuous))
    }
}
