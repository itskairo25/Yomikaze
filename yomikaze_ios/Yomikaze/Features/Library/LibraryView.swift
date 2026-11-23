import SwiftData
import SwiftUI

enum LibrarySort: String, CaseIterable, Identifiable {
    case recentlyAdded
    case lastRead
    case title

    var id: String { rawValue }

    var name: String {
        switch self {
        case .recentlyAdded: return tr("Recently added")
        case .lastRead: return tr("Last read")
        case .title: return tr("Title")
        }
    }
}

enum LibrarySection: String, CaseIterable, Identifiable {
    case saved
    case downloads

    var id: String { rawValue }

    var title: String {
        switch self {
        case .saved: return tr("Saved")
        case .downloads: return tr("Downloads")
        }
    }
}

struct DownloadGroup: Identifiable {
    let id: String
    let title: String
    let chapterCount: Int
}

struct LibraryView: View {
    @Environment(AppRouter.self) private var router
    @Environment(\.modelContext) private var context

    @Query(filter: #Predicate<SavedManga> { $0.inLibrary }, sort: \SavedManga.dateAdded, order: .reverse)
    private var library: [SavedManga]
    @Query(sort: \DownloadedChapter.downloadedAt, order: .reverse)
    private var downloads: [DownloadedChapter]

    @AppStorage(SettingsKey.librarySort) private var sortRaw = LibrarySort.recentlyAdded.rawValue
    @AppStorage(SettingsKey.libraryCategories) private var categoriesRaw = ""
    // empty means all
    @State private var selectedCategory = ""
    @State private var showNewCategory = false
    @State private var newCategoryName = ""
    @Namespace private var zoomNamespace

    private let network = NetworkMonitor.shared
    private let downloader = DownloadManager.shared
    private let columns = [GridItem(.adaptive(minimum: 100, maximum: 170), spacing: 14, alignment: .top)]

    private var sorted: [SavedManga] {
        switch LibrarySort(rawValue: sortRaw) ?? .recentlyAdded {
        case .recentlyAdded:
            return library
        case .lastRead:
            return library.sorted { $0.lastReadAt > $1.lastReadAt }
        case .title:
            return library.sorted { $0.title.localizedCaseInsensitiveCompare($1.title) == .orderedAscending }
        }
    }

    private var categories: [String] {
        categoriesRaw.split(separator: "\n").map(String.init)
    }

    private var filtered: [SavedManga] {
        selectedCategory.isEmpty ? sorted : sorted.filter { $0.category == selectedCategory }
    }

    private var downloadGroups: [DownloadGroup] {
        var order: [String] = []
        var titles: [String: String] = [:]
        var counts: [String: Int] = [:]
        for item in downloads {
            if counts[item.mangaID] == nil {
                order.append(item.mangaID)
                titles[item.mangaID] = item.mangaTitle
            }
            counts[item.mangaID, default: 0] += 1
        }
        return order.map { DownloadGroup(id: $0, title: titles[$0] ?? "", chapterCount: counts[$0] ?? 0) }
    }

    // kept in the router so the app can jump to downloads when offline
    private var section: LibrarySection {
        router.librarySection
    }

    var body: some View {
        ScrollView {
            LazyVStack(spacing: 18) {
                Picker(tr("Section"), selection: Binding(get: { router.librarySection }, set: { router.librarySection = $0 })) {
                    ForEach(LibrarySection.allCases) { option in
                        Text(option.title).tag(option)
                    }
                }
                .pickerStyle(.segmented)

                if !network.isOnline {
                    OfflineBanner()
                }

                switch section {
                case .saved:
                    savedContent
                case .downloads:
                    downloadsContent
                }
            }
            .padding(.horizontal, 20)
            .padding(.top, 8)
            .padding(.bottom, 24)
        }
        .background {
            AmbientBackground()
        }
        .navigationTitle(tr("Library"))
        .toolbar {
            if section == .saved {
                ToolbarItem(placement: .topBarTrailing) {
                    Menu {
                        Picker(tr("Sort by"), selection: $sortRaw) {
                            ForEach(LibrarySort.allCases) { sort in
                                Text(sort.name).tag(sort.rawValue)
                            }
                        }
                    } label: {
                        Image(systemName: "arrow.up.arrow.down")
                    }
                }
            }
        }
        .navigationDestination(for: MangaSummary.self) { manga in
            MangaDetailView(manga: manga)
                .navigationTransition(.zoom(sourceID: manga.id, in: zoomNamespace))
        }
        .environment(\.zoomNamespace, zoomNamespace)
    }

    @ViewBuilder
    private var savedContent: some View {
        if library.isEmpty {
            ContentUnavailableView {
                Label(tr("Your library is empty"), systemImage: "books.vertical")
            } description: {
                Text(tr("Tap the bookmark on any series to keep it here. Everything is stored on this iPhone."))
            } actions: {
                Button(tr("Browse manga")) {
                    router.selectedTab = .browse
                }
                .buttonStyle(.glassProminent)
            }
            .padding(.top, 40)
        } else {
            categoryBar

            let items = filtered
            if items.isEmpty {
                ContentUnavailableView {
                    Label(tr("Nothing in {category}", ["category": selectedCategory]), systemImage: "folder")
                } description: {
                    Text(tr("Touch and hold a series, then choose Move to category."))
                }
                .padding(.top, 30)
            }

            LazyVGrid(columns: columns, spacing: 18) {
                ForEach(items) { item in
                    NavigationLink(value: item.summary) {
                        MangaCard(
                            manga: item.summary,
                            subtitle: item.hasHistory ? item.lastChapterName : tr("Not started")
                        )
                    }
                    .buttonStyle(.plain)
                    .contextMenu {
                        if item.hasHistory {
                            Button {
                                router.resume(item)
                            } label: {
                                Label(tr("Continue reading"), systemImage: "play.fill")
                            }
                        }
                        Menu {
                            Button {
                                item.category = ""
                            } label: {
                                Label(tr("None"), systemImage: item.category.isEmpty ? "checkmark" : "minus")
                            }
                            ForEach(categories, id: \.self) { name in
                                Button {
                                    item.category = name
                                } label: {
                                    Label(name, systemImage: item.category == name ? "checkmark" : "folder")
                                }
                            }
                        } label: {
                            Label(tr("Move to category"), systemImage: "folder")
                        }
                        Button(role: .destructive) {
                            item.inLibrary = false
                            if !item.hasHistory {
                                context.delete(item)
                            }
                        } label: {
                            Label(tr("Remove from library"), systemImage: "bookmark.slash")
                        }
                    }
                }
            }
        }
    }

    private var categoryBar: some View {
        ScrollView(.horizontal) {
            HStack(spacing: 8) {
                categoryChip(title: tr("All"), systemImage: "square.grid.2x2.fill", name: "")

                ForEach(categories, id: \.self) { name in
                    categoryChip(title: name, systemImage: "folder.fill", name: name)
                        .contextMenu {
                            Button(role: .destructive) {
                                deleteCategory(name)
                            } label: {
                                Label(tr("Delete category"), systemImage: "trash")
                            }
                        }
                }

                Button {
                    newCategoryName = ""
                    showNewCategory = true
                } label: {
                    Image(systemName: "plus")
                        .font(.subheadline.weight(.semibold))
                        .frame(width: 36, height: 36)
                        .background(.quaternary, in: Circle())
                }
                .buttonStyle(.plain)
            }
        }
        .scrollIndicators(.hidden)
        .scrollClipDisabled()
        .alert(tr("New category"), isPresented: $showNewCategory) {
            TextField(tr("Name"), text: $newCategoryName)
            Button(tr("Add")) {
                addCategory(newCategoryName)
            }
            Button(tr("Cancel"), role: .cancel) {}
        } message: {
            Text(tr("For example Reading, Finished or Later."))
        }
    }

    private func categoryChip(title: String, systemImage: String, name: String) -> some View {
        let selected = selectedCategory == name
        let count = name.isEmpty ? library.count : library.filter { $0.category == name }.count

        return Button {
            withAnimation(.snappy(duration: 0.25)) {
                selectedCategory = name
            }
        } label: {
            HStack(spacing: 6) {
                Image(systemName: systemImage)
                    .font(.caption)
                Text(title)
                    .font(.subheadline.weight(.semibold))
                    .lineLimit(1)
                Text("\(count)")
                    .font(.caption.weight(.semibold).monospacedDigit())
                    .opacity(0.7)
            }
            .foregroundStyle(selected ? AnyShapeStyle(.white) : AnyShapeStyle(.primary))
            .padding(.horizontal, 14)
            .frame(height: 36)
            .background(selected ? AnyShapeStyle(.tint) : AnyShapeStyle(.quaternary), in: Capsule())
            .contentShape(Capsule())
        }
        .buttonStyle(.plain)
    }

    private func addCategory(_ raw: String) {
        let name = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !name.isEmpty, !categories.contains(where: { $0.caseInsensitiveCompare(name) == .orderedSame }) else { return }
        categoriesRaw = (categories + [name]).joined(separator: "\n")
        selectedCategory = name
    }

    // series in the category stay in the library without one
    private func deleteCategory(_ name: String) {
        for item in library where item.category == name {
            item.category = ""
        }
        categoriesRaw = categories.filter { $0 != name }.joined(separator: "\n")
        if selectedCategory == name {
            selectedCategory = ""
        }
    }

    @ViewBuilder
    private var downloadsContent: some View {
        let groups = downloadGroups

        if downloader.activeCount > 0 {
            Label(tr("Downloading {n} chapters...", ["n": downloader.activeCount]), systemImage: "arrow.down.circle")
                .font(.footnote.weight(.medium))
                .foregroundStyle(.secondary)
                .frame(maxWidth: .infinity, alignment: .leading)
        }

        if groups.isEmpty {
            ContentUnavailableView {
                Label(tr("No downloads yet"), systemImage: "arrow.down.circle")
            } description: {
                Text(tr("Download chapters from a series page to read them without a connection."))
            }
            .padding(.top, 40)
        } else {
            ForEach(groups) { group in
                HStack(spacing: 4) {
                    NavigationLink(value: MangaSummary(id: group.id, title: group.title)) {
                        MangaRow(
                            manga: MangaSummary(id: group.id, title: group.title),
                            detail: group.chapterCount == 1 ? tr("1 chapter downloaded") : tr("{n} chapters downloaded", ["n": group.chapterCount])
                        )
                    }
                    .buttonStyle(.plain)

                    Menu {
                        Button(role: .destructive) {
                            downloader.deleteAll(mangaID: group.id, context: context)
                        } label: {
                            Label(tr("Delete downloads"), systemImage: "trash")
                        }
                    } label: {
                        Image(systemName: "ellipsis")
                            .font(.body.weight(.semibold))
                            .foregroundStyle(.secondary)
                            .frame(width: 40, height: 40)
                            .contentShape(Rectangle())
                    }
                }
            }
        }
    }
}
