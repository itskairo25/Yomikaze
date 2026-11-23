import PhotosUI
import SwiftData
import SwiftUI
import UniformTypeIdentifiers

struct SettingsView: View {
    @Environment(\.modelContext) private var context
    @Environment(AppRouter.self) private var router

    private let network = NetworkMonitor.shared

    @AppStorage(SettingsKey.accent) private var accentRaw = AccentChoice.yoru.rawValue
    @AppStorage(SettingsKey.appearance) private var appearanceRaw = AppearanceChoice.system.rawValue
    @AppStorage(SettingsKey.readerMode) private var readerModeRaw = ReaderMode.vertical.rawValue
    @AppStorage(SettingsKey.keepAwake) private var keepAwake = true
    @AppStorage(SettingsKey.showAdult) private var showAdult = false
    @AppStorage(SettingsKey.appLock) private var appLock = false

    @Query private var downloads: [DownloadedChapter]
    @Query(filter: #Predicate<SavedManga> { $0.inLibrary }) private var library: [SavedManga]

    @State private var cacheSize: Int64 = 0
    @State private var downloadsSize: Int64 = 0
    @State private var confirmDeleteDownloads = false
    @State private var showExporter = false
    @State private var showImporter = false
    @State private var exportDocument = BackupDocument(data: Data())
    @State private var exportSummary = ""
    @State private var pendingImport: BackupFile?
    @State private var backupMessage: String?
    @State private var wallpaperItem: PhotosPickerItem?
    @State private var showWallpaperEditor = false
    @State private var isLoadingWallpaper = false
    @AppStorage(SettingsKey.wallpaperZoom) private var wallpaperZoom = 1.0
    @AppStorage(SettingsKey.wallpaperX) private var wallpaperX = 0.0
    @AppStorage(SettingsKey.wallpaperY) private var wallpaperY = 0.0
    @AppStorage(SettingsKey.wallpaperSource) private var wallpaperSource = ""

    @AppStorage(SettingsKey.customAccent) private var customAccentHex = AppTheme.defaultCustomAccent
    @AppStorage(SettingsKey.wallpaperDim) private var wallpaperDim = 0.6

    private let wallpaper = WallpaperStore.shared

    private var customAccentBinding: Binding<Color> {
        Binding(
            get: { Color(hex: customAccentHex) ?? AccentChoice.yoru.color },
            set: { color in
                customAccentHex = color.hexString
                accentRaw = AppTheme.customAccentID
            }
        )
    }

    private var appVersion: String {
        Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? "-"
    }

    var body: some View {
        Form {
            Section {
                HStack(spacing: 14) {
                    Text("読")
                        .font(.system(size: 30, weight: .heavy))
                        .foregroundStyle(.white)
                        .frame(width: 58, height: 58)
                        .background(.tint, in: RoundedRectangle(cornerRadius: 15, style: .continuous))
                    VStack(alignment: .leading, spacing: 2) {
                        // "yomi" in the text colour and "kaze" (wind) in the accent
                        Text("\(Text("Yomi").foregroundStyle(.primary))\(Text("kaze").foregroundStyle(.tint))")
                            .font(.title2.weight(.heavy))
                            .kerning(0.4)
                        Text("読み風 · v\(appVersion)")
                            .font(.subheadline.weight(.medium))
                            .foregroundStyle(.secondary)
                    }
                }
                .padding(.vertical, 4)
            }

            Section {
                NavigationLink {
                    appearancePage
                } label: {
                    row(tr("Appearance"), "paintpalette.fill", .purple)
                }
                NavigationLink {
                    backgroundsPage
                } label: {
                    row(tr("Backgrounds"), "photo.on.rectangle.angled", .pink)
                }
                NavigationLink {
                    page(tr("Reader")) {
                        Section {
                            ReaderOptions()
                        } footer: {
                            Text(tr("The same options are behind the gear button inside the reader."))
                        }
                    }
                } label: {
                    row(tr("Reader"), "book.fill", .orange)
                }
                NavigationLink {
                    page("AniList") {
                        AniListSection()
                    }
                } label: {
                    row("AniList", "arrow.triangle.2.circlepath", .blue, value: AniListSync.shared.user?.name ?? tr("Not connected"))
                }
            }

            Section {
                NavigationLink {
                    libraryPage
                } label: {
                    row(tr("Library and storage"), "books.vertical.fill", .green, value: tr("{n} series", ["n": library.count]))
                }
                NavigationLink {
                    backupPage
                } label: {
                    row(tr("Backup"), "arrow.left.arrow.right", .teal)
                }
                NavigationLink {
                    privacyPage
                } label: {
                    row(tr("Privacy"), "lock.fill", .indigo)
                }
            }

            Section {
                NavigationLink {
                    aboutPage
                } label: {
                    row(tr("About"), "info.circle.fill", .gray)
                }
            }
        }
        .scrollContentBackground(.hidden)
        .background {
            AmbientBackground()
        }
        .navigationTitle(tr("Settings"))
        .navigationDestination(for: MangaSummary.self) { manga in
            MangaDetailView(manga: manga)
        }
        .task {
            await refreshSizes()
        }
        .onChange(of: downloads.count) { _, _ in
            Task { await refreshSizes() }
        }
    }

    // a settings row: a coloured icon tile, the title and an optional value on the right
    private func row(_ title: String, _ icon: String, _ color: Color, value: String? = nil) -> some View {
        HStack(spacing: 12) {
            Image(systemName: icon)
                .font(.footnote.weight(.semibold))
                .foregroundStyle(.white)
                .frame(width: 30, height: 30)
                .background(color.gradient, in: RoundedRectangle(cornerRadius: 8, style: .continuous))
            Text(title)
            if let value {
                Spacer(minLength: 8)
                Text(value)
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
            }
        }
    }

    private func page<Content: View>(_ title: String, @ViewBuilder content: () -> Content) -> some View {
        Form {
            content()
        }
        .scrollContentBackground(.hidden)
        .background {
            AmbientBackground()
        }
        .navigationTitle(title)
        .navigationBarTitleDisplayMode(.inline)
    }

    private var appearancePage: some View {
        page(tr("Appearance")) {
            Section {
                Picker(tr("Theme"), selection: $appearanceRaw) {
                    ForEach(AppearanceChoice.allCases) { choice in
                        Text(choice.name).tag(choice.rawValue)
                    }
                }

                Picker(tr("Language"), selection: Binding(get: { AppLanguage.shared.setting }, set: { AppLanguage.shared.setting = $0 })) {
                    ForEach(L10n.languages) { item in
                        Text(item.code == "system" ? tr(item.name) : item.name).tag(item.code)
                    }
                }
            }

            Section(tr("Accent colour")) {
                HStack(spacing: 14) {
                    ForEach(AccentChoice.allCases) { choice in
                        Button {
                            withAnimation(.snappy) {
                                accentRaw = choice.rawValue
                            }
                        } label: {
                            VStack(spacing: 6) {
                                Circle()
                                    .fill(choice.color)
                                    .frame(width: 36, height: 36)
                                    .overlay {
                                        if accentRaw == choice.rawValue {
                                            Image(systemName: "checkmark")
                                                .font(.footnote.weight(.bold))
                                                .foregroundStyle(.white)
                                        }
                                    }
                                Text(choice.name)
                                    .font(.caption2)
                                    .foregroundStyle(.secondary)
                            }
                        }
                        // borderless keeps each swatch tappable inside a form row
                        .buttonStyle(.borderless)
                    }
                }
                .padding(.vertical, 4)

                ColorPicker(selection: customAccentBinding, supportsOpacity: false) {
                    HStack(spacing: 8) {
                        Text(tr("Custom colour"))
                        if accentRaw == AppTheme.customAccentID {
                            Image(systemName: "checkmark")
                                .font(.footnote.weight(.bold))
                                .foregroundStyle(.tint)
                        }
                    }
                }
            }
        }
    }

    private var backgroundsPage: some View {
        page(tr("Backgrounds")) {
            Section {
                PhotosPicker(selection: $wallpaperItem, matching: .images) {
                    HStack {
                        Label(wallpaper.image == nil ? tr("Choose background image") : tr("Change background image"), systemImage: "photo")
                        if isLoadingWallpaper {
                            Spacer()
                            ProgressView()
                        }
                    }
                }

                if let image = wallpaper.image {
                    Image(uiImage: image)
                        .resizable()
                        .scaledToFill()
                        .frame(height: 120)
                        .frame(maxWidth: .infinity)
                        .clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
                        .allowsHitTesting(false)

                    Button {
                        showWallpaperEditor = true
                    } label: {
                        Label(tr("Adjust position and zoom"), systemImage: "crop")
                    }

                    VStack(alignment: .leading, spacing: 4) {
                        Text(tr("Dimming"))
                        Slider(value: $wallpaperDim, in: 0.2...0.9)
                    }

                    Button {
                        if let color = wallpaper.matchingAccent {
                            customAccentHex = color.hexString
                            accentRaw = AppTheme.customAccentID
                        }
                    } label: {
                        Label(tr("Match accent colour to image"), systemImage: "eyedropper")
                    }

                    Button(role: .destructive) {
                        wallpaper.clear()
                        wallpaperSource = ""
                        wallpaperItem = nil
                    } label: {
                        Label(tr("Remove background image"), systemImage: "trash")
                    }
                }
            } header: {
                Text(tr("Background"))
            } footer: {
                Text(tr("Shown behind Library, Browse, Search, History and Settings. The accent colour is matched to the image when you pick one."))
            }
            Section {
                BackgroundGallery()
            } header: {
                Text(tr("Anime backgrounds"))
            } footer: {
                Text(tr("Anime wallpapers from Wallhaven, safe for work and sharp on your screen. Only the one you pick is downloaded, so it works offline too."))
            }
        }
        .onChange(of: wallpaperItem) { _, item in
            guard let item else { return }
            Task {
                isLoadingWallpaper = true
                defer { isLoadingWallpaper = false }
                guard let data = try? await item.loadTransferable(type: Data.self) else { return }
                if let color = await wallpaper.set(data) {
                    wallpaperSource = ""
                    wallpaperZoom = 1
                    wallpaperX = 0
                    wallpaperY = 0
                    customAccentHex = color.hexString
                    accentRaw = AppTheme.customAccentID
                }
            }
        }
        .sheet(isPresented: $showWallpaperEditor) {
            if let image = wallpaper.image {
                WallpaperEditor(image: image)
            }
        }
    }

    private var libraryPage: some View {
        page(tr("Library and storage")) {
            Section(tr("Content")) {
                Toggle(tr("Show adult content"), isOn: $showAdult)
            }

            Section {
                Toggle(isOn: Binding(
                    get: { network.forcedOffline },
                    set: { enabled in
                        network.forcedOffline = enabled
                        if enabled {
                            router.showDownloads()
                        }
                    }
                )) {
                    Label(tr("Offline mode"), systemImage: "wifi.slash")
                }
            } header: {
                Text(tr("Connection"))
            } footer: {
                Text(tr("Uses no data: only your downloaded chapters and saved series are shown."))
            }

            Section {
                LabeledContent(tr("Library"), value: tr("{n} series", ["n": library.count]))
                LabeledContent(tr("Downloaded chapters"), value: "\(downloads.count) · \(downloadsSize.formattedByteCount)")
                LabeledContent(tr("Image cache"), value: cacheSize.formattedByteCount)

                Button(tr("Clear image cache")) {
                    Task {
                        await ImageStore.shared.clearCache()
                        ImageLoader.clearMemory()
                        await refreshSizes()
                    }
                }
                Button(tr("Delete all downloads"), role: .destructive) {
                    confirmDeleteDownloads = true
                }
                .disabled(downloads.isEmpty)
            } header: {
                Text(tr("Storage"))
            } footer: {
                Text(tr("Your library, reading progress, history and downloads are stored only on this iPhone."))
            }
        }
        .confirmationDialog(tr("Delete all downloaded chapters?"), isPresented: $confirmDeleteDownloads, titleVisibility: .visible) {
            Button(tr("Delete downloads"), role: .destructive) {
                DownloadManager.shared.deleteAll(context: context)
                Task {
                    try? await Task.sleep(nanoseconds: 300_000_000)
                    await refreshSizes()
                }
            }
        }
    }

    private var backupPage: some View {
        page(tr("Backup")) {
            Section {
                Button {
                    exportBackup()
                } label: {
                    Label(tr("Export progress"), systemImage: "square.and.arrow.up")
                }
                // each file sheet sits on its own button, two on one view can clash
                .fileExporter(
                    isPresented: $showExporter,
                    document: exportDocument,
                    contentType: .json,
                    defaultFilename: BackupFile.fileName()
                ) { result in
                    switch result {
                    case .success:
                        backupMessage = exportSummary
                    case .failure(let error):
                        backupMessage = tr("Couldn't save the backup: {error}", ["error": error.localizedDescription])
                    }
                }

                Button {
                    showImporter = true
                } label: {
                    Label(tr("Import"), systemImage: "square.and.arrow.down")
                }
                .fileImporter(isPresented: $showImporter, allowedContentTypes: [.json]) { result in
                    readBackup(result)
                }
            } footer: {
                Text(tr("Saves your library, categories, history and the page you're on in every chapter to one file. Open it in Yomikaze on Windows or another iPhone to carry on where you left off. Downloaded pages aren't included."))
            }
        }
        .confirmationDialog(
            tr("Import backup?"),
            isPresented: Binding(get: { pendingImport != nil }, set: { if !$0 { pendingImport = nil } }),
            titleVisibility: .visible,
            presenting: pendingImport
        ) { file in
            Button(tr("Import progress")) {
                importBackup(file, withSettings: false)
            }
            if file.settings != nil {
                Button(tr("Import progress and settings")) {
                    importBackup(file, withSettings: true)
                }
            }
            Button(tr("Cancel"), role: .cancel) {
                pendingImport = nil
            }
        } message: { file in
            let inLibrary = file.manga.filter(\.inLibrary).count
            Text(tr("From Yomikaze {version} on {from}: {series} series ({library} in the library) and {chapters} chapter positions.", ["version": file.appVersion, "from": file.sourceName, "series": file.manga.count, "library": inLibrary, "chapters": file.progress.count]) + " " + tr("Nothing is deleted: your library is merged with the backup, and for every chapter the most recent page is kept."))
        }
        .alert(
            tr("Backup"),
            isPresented: Binding(get: { backupMessage != nil }, set: { if !$0 { backupMessage = nil } })
        ) {
            Button(tr("OK")) {
                backupMessage = nil
            }
        } message: {
            Text(backupMessage ?? "")
        }
    }

    private var privacyPage: some View {
        page(tr("Privacy")) {
            Section {
                Toggle(isOn: Binding(
                    get: { appLock },
                    set: { enabled in
                        // turning the lock on or off asks for face id itself
                        Task {
                            if await AppLock.authenticate(reason: enabled ? tr("Turn on the app lock") : tr("Turn off the app lock")) {
                                appLock = enabled
                            }
                        }
                    }
                )) {
                    Label(tr("Lock with Face ID or passcode"), systemImage: "lock.fill")
                }
            } footer: {
                Text(tr("Asks for Face ID, Touch ID or your iPhone passcode every time the app is opened."))
            }
        }
    }

    private var aboutPage: some View {
        page(tr("About")) {
            Section {
                NavigationLink {
                    ChangelogView()
                } label: {
                    Label(tr("What's New"), systemImage: "sparkles")
                }
                NavigationLink {
                    CreditsView()
                } label: {
                    Label(tr("Credits"), systemImage: "person.2.fill")
                }
            }
            Section {
                LabeledContent(tr("Version"), value: appVersion)
                LabeledContent(tr("Source"), value: "WeebCentral")
                Link(tr("Open weebcentral.com"), destination: URL(string: WeebCentral.base)!)
            }
        }
    }

    private func exportBackup() {
        let file = BackupService.makeBackup(context: context)
        do {
            exportDocument = BackupDocument(data: try file.encoded())
            exportSummary = tr("Saved {series} series and {chapters} chapter positions", ["series": file.manga.count, "chapters": file.progress.count])
            showExporter = true
        } catch {
            backupMessage = tr("Couldn't create the backup: {error}", ["error": error.localizedDescription])
        }
    }

    private func readBackup(_ result: Result<URL, Error>) {
        switch result {
        case .success(let url):
            // files from icloud drive or other apps need scoped access
            let scoped = url.startAccessingSecurityScopedResource()
            defer {
                if scoped {
                    url.stopAccessingSecurityScopedResource()
                }
            }
            do {
                let data = try Data(contentsOf: url)
                pendingImport = try BackupFile.parse(data)
            } catch {
                backupMessage = (error as? LocalizedError)?.errorDescription ?? error.localizedDescription
            }
        case .failure(let error):
            backupMessage = error.localizedDescription
        }
    }

    private func importBackup(_ file: BackupFile, withSettings: Bool) {
        let result = BackupService.apply(file, context: context, withSettings: withSettings)
        pendingImport = nil
        backupMessage = tr("Imported {series} series and {chapters} chapter positions", ["series": result.series, "chapters": result.chapters])
    }

    private func refreshSizes() async {
        cacheSize = await ImageStore.shared.cacheSize()
        downloadsSize = await ImageStore.shared.downloadsSize()
    }
}
