import SwiftUI

// anime wallpapers by popularity, famous series and genre. tapping one downloads it and makes
// it the background, the picture stays on the device so it also works offline.
struct BackgroundGallery: View {
    @AppStorage(SettingsKey.accent) private var accentRaw = AccentChoice.yoru.rawValue
    @AppStorage(SettingsKey.customAccent) private var customAccentHex = AppTheme.defaultCustomAccent
    @AppStorage(SettingsKey.wallpaperZoom) private var wallpaperZoom = 1.0
    @AppStorage(SettingsKey.wallpaperX) private var wallpaperX = 0.0
    @AppStorage(SettingsKey.wallpaperY) private var wallpaperY = 0.0
    @AppStorage(SettingsKey.wallpaperSource) private var wallpaperSource = ""

    // 0 is popular, 1 the famous series, then the genres
    @State private var tab = 0
    @State private var series = 0
    @State private var failure: String?

    private let gallery = WallpaperGallery.shared
    private let wallpaper = WallpaperStore.shared

    private var tabNames: [String] {
        ["Popular", "Famous series"] + WallpaperGallery.genres
    }

    private var seriesNames: [String] {
        if tab == 1 {
            return WallpaperGallery.famous
        }
        guard tab >= 2, let sets = gallery.genreSeries, tab - 2 < sets.count else { return [] }
        return sets[tab - 2]
    }

    private var query: String {
        guard tab > 0, !seriesNames.isEmpty else { return "" }
        return seriesNames[min(series, seriesNames.count - 1)]
    }

    private var isPad: Bool {
        UIDevice.current.userInterfaceIdiom == .pad
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            chips(tabNames.map { tr($0) }, selected: tab) { index in
                tab = index
                series = 0
            }

            if tab >= 1 {
                if seriesNames.isEmpty {
                    seriesPlaceholder
                } else {
                    Text(tr("Series"))
                        .font(.caption.weight(.semibold))
                        .foregroundStyle(.secondary)
                    chips(seriesNames, selected: min(series, seriesNames.count - 1), small: true) { index in
                        series = index
                    }
                }
            }

            if tab == 0 || !seriesNames.isEmpty {
                results
            }
        }
        .padding(.vertical, 6)
        .task(id: tab) {
            if tab >= 2 {
                await gallery.loadSeries()
            }
        }
        .task(id: query + "\(tab)") {
            if tab == 0 || !seriesNames.isEmpty {
                await gallery.load(query)
            }
        }
        .alert(tr("Couldn't download that picture: {error}", ["error": failure ?? ""]), isPresented: Binding(get: { failure != nil }, set: { if !$0 { failure = nil } })) {
            Button(tr("OK"), role: .cancel) {}
        }
    }

    // a row of choices that scrolls sideways, so it never takes over the screen on a phone
    private func chips(_ names: [String], selected: Int, small: Bool = false, choose: @escaping (Int) -> Void) -> some View {
        ScrollViewReader { proxy in
            ScrollView(.horizontal) {
                HStack(spacing: 8) {
                    ForEach(Array(names.enumerated()), id: \.offset) { index, name in
                        Button {
                            withAnimation(.snappy) {
                                choose(index)
                            }
                        } label: {
                            Text(name)
                                .font((small ? Font.caption : Font.subheadline).weight(.semibold))
                                .padding(.horizontal, small ? 12 : 14)
                                .padding(.vertical, small ? 7 : 9)
                                .foregroundStyle(index == selected ? Color.white : Color.primary)
                                .background(index == selected ? AnyShapeStyle(.tint) : AnyShapeStyle(.quaternary), in: Capsule())
                        }
                        .buttonStyle(.plain)
                        .id(index)
                    }
                }
                .padding(.horizontal, 2)
            }
            .scrollIndicators(.hidden)
            .onChange(of: selected) { _, value in
                withAnimation(.snappy) {
                    proxy.scrollTo(value, anchor: .center)
                }
            }
        }
    }

    @ViewBuilder
    private var seriesPlaceholder: some View {
        if gallery.loadingSeries || gallery.genreSeries == nil && gallery.seriesError == nil {
            ProgressView()
                .frame(maxWidth: .infinity)
                .padding(.vertical, 20)
        } else {
            VStack(spacing: 10) {
                Text(gallery.seriesError ?? tr("Nothing found"))
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
                Button(tr("Try again")) {
                    Task { await gallery.retrySeries() }
                }
                .buttonStyle(.bordered)
            }
            .frame(maxWidth: .infinity)
        }
    }

    private var results: some View {
        let search = gallery.search(query)
        // portrait tiles on a phone, wide ones on an ipad, like the pictures themselves
        let columns = [GridItem(.adaptive(minimum: isPad ? 220 : 100), spacing: 10)]
        return VStack(spacing: 14) {
            LazyVGrid(columns: columns, spacing: 10) {
                ForEach(search.items) { item in
                    tile(item)
                }
            }
            if search.loading {
                ProgressView()
                    .padding(.vertical, 10)
            } else if let error = search.error {
                VStack(spacing: 10) {
                    Text(error)
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                    Button(tr("Try again")) {
                        Task { await gallery.retry(query) }
                    }
                    .buttonStyle(.bordered)
                }
            } else if search.page > 0 && search.items.isEmpty {
                Text(tr("Nothing found"))
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
            } else if search.hasMore {
                Button {
                    Task { await gallery.load(query, more: true) }
                } label: {
                    Label(tr("Load more"), systemImage: "chevron.down")
                }
                .buttonStyle(.bordered)
            }
        }
        .frame(maxWidth: .infinity)
    }

    private func tile(_ item: Wallpaper) -> some View {
        let selected = wallpaperSource == item.full.absoluteString && wallpaper.image != nil
        return Button {
            pick(item)
        } label: {
            RemoteImage(url: item.thumb, maxWidth: 400)
                .aspectRatio(isPad ? 16.0 / 9.0 : 9.0 / 16.0, contentMode: .fit)
                .clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
                .overlay {
                    RoundedRectangle(cornerRadius: 12, style: .continuous)
                        .strokeBorder(selected ? AnyShapeStyle(.tint) : AnyShapeStyle(.quaternary), lineWidth: selected ? 3 : 1)
                }
                .overlay(alignment: .topTrailing) {
                    if selected {
                        Image(systemName: "checkmark.circle.fill")
                            .font(.title3)
                            .foregroundStyle(.white, .tint)
                            .padding(6)
                    }
                }
                .overlay {
                    if gallery.busy == item.full {
                        ZStack {
                            RoundedRectangle(cornerRadius: 12, style: .continuous)
                                .fill(.black.opacity(0.45))
                            ProgressView()
                                .tint(.white)
                        }
                    }
                }
        }
        .buttonStyle(.plain)
        .disabled(gallery.busy != nil)
        .accessibilityLabel(item.resolution)
    }

    private func pick(_ item: Wallpaper) {
        Task {
            do {
                let data = try await gallery.picture(for: item.full)
                if let color = await wallpaper.set(data) {
                    wallpaperZoom = 1
                    wallpaperX = 0
                    wallpaperY = 0
                    wallpaperSource = item.full.absoluteString
                    customAccentHex = color.hexString
                    accentRaw = AppTheme.customAccentID
                }
            } catch {
                failure = error.localizedDescription
            }
        }
    }
}
