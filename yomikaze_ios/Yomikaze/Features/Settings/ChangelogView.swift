import SwiftUI

// add new releases at the top
enum Changelog {
    struct Release: Identifiable {
        let version: String
        let title: String
        let changes: [Change]

        var id: String { version }
    }

    struct Change: Identifiable {
        let icon: String
        let text: String

        var id: String { text }
    }

    static let releases: [Release] = [
        Release(version: "0.2.7", title: "AniList and new languages", changes: [
            Change(icon: "arrow.triangle.2.circlepath", text: "AniList sync: connect your account, your manga list comes into the library, and reading progress, the library and favourites stay in sync"),
            Change(icon: "character.bubble.fill", text: "Yomikaze now speaks Hungarian, Spanish, Japanese and Chinese, and there's a pirate language too"),
            Change(icon: "star.fill", text: "Finished a series? It moves to Completed on AniList and you can rate it there"),
            Change(icon: "ipad.landscape", text: "iPad support: a sidebar, layouts that use the bigger screen, keyboard shortcuts and two pages side by side in landscape"),
            Change(icon: "gearshape.fill", text: "Settings are sorted into categories"),
            Change(icon: "hare.fill", text: "Smoother page turning, especially on iPad"),
            Change(icon: "photo.on.rectangle.angled", text: "Backgrounds: anime wallpapers by popularity, famous series and genre, the one you pick is saved on your device so it works offline too"),
            Change(icon: "list.bullet", text: "Reader: a chapter list, a progress line and the page number after each turn, and the next chapter is ready before you get there"),
            Change(icon: "iphone", text: "On iPhone Yomikaze stays upright, made for reading in portrait"),
            Change(icon: "wrench.and.screwdriver.fill", text: "Various bug fixes"),
        ]),
        Release(version: "0.2.6", title: "Stay in sync", changes: [
            Change(icon: "arrow.left.arrow.right", text: "Backups: export your library and the page you're on in every chapter, and import it on Windows or another iPhone"),
            Change(icon: "eye.slash", text: "Distraction-free reading: the controls stay hidden until you tap the small button in the corner"),
            Change(icon: "hand.tap.fill", text: "Fixed: the controls button in the reader turned the page instead of showing the controls"),
        ]),
        Release(version: "0.2.5", title: "Read your way", changes: [
            Change(icon: "hand.tap.fill", text: "Tap zones: tap the left or right edge to turn the page"),
            Change(icon: "gearshape.fill", text: "Reader options: background colour, page dimming and a gap between pages, right inside the reader"),
            Change(icon: "folder.fill", text: "Library categories: make your own and sort your series into them"),
            Change(icon: "lock.fill", text: "Lock the app with Face ID or your passcode"),
            Change(icon: "photo.on.rectangle", text: "Share a series as a picture card"),
            Change(icon: "arrow.up.left.and.arrow.down.right", text: "Covers zoom smoothly into the series page from Library, Browse and Search"),
        ]),
        Release(version: "0.2.4", title: "Make it yours", changes: [
            Change(icon: "chart.bar.fill", text: "Statistics: last read, totals, streak, the last 7 days and progress for every series"),
            Change(icon: "rectangle.3.group.fill", text: "Redesigned series page with a centred poster, fact strip, progress ring and chapter cards"),
            Change(icon: "photo.fill", text: "Banners on series pages"),
            Change(icon: "safari.fill", text: "Redesigned Browse with ranked top picks"),
            Change(icon: "paintpalette.fill", text: "Custom accent colour"),
            Change(icon: "photo.on.rectangle.angled", text: "Your own background image, with position, zoom and dimming"),
            Change(icon: "wifi.slash", text: "Offline mode switch; the app opens your downloads when there is no connection"),
            Change(icon: "magnifyingglass", text: "New search field that actually finds things"),
            Change(icon: "person.2.fill", text: "Credits page"),
            Change(icon: "sparkles", text: "Launch animation and this What's New screen"),
        ]),
        Release(version: "0.2.1", title: "New look", changes: [
            Change(icon: "app.fill", text: "New app icon"),
        ]),
        Release(version: "0.2.0", title: "Offline reading", changes: [
            Change(icon: "arrow.down.circle.fill", text: "Downloads section and download all / unread chapters"),
            Change(icon: "square.grid.3x3.fill", text: "Browse feeds: Hot, Latest, Popular and New"),
            Change(icon: "tag.fill", text: "Genre tiles with icons"),
            Change(icon: "bolt.fill", text: "Faster scrolling and page loading"),
        ]),
        Release(version: "0.1.0", title: "First build", changes: [
            Change(icon: "books.vertical.fill", text: "Library, Browse, Search, History and Settings"),
            Change(icon: "book.pages.fill", text: "Reader with vertical and paged modes"),
            Change(icon: "drop.fill", text: "Liquid Glass design"),
        ]),
    ]

    private static let seenKey = "lastSeenBuild"

    static var currentBuild: String {
        Bundle.main.object(forInfoDictionaryKey: "CFBundleVersion") as? String ?? "0"
    }

    // true on the first run after an install or update
    static var shouldPresent: Bool {
        UserDefaults.standard.string(forKey: seenKey) != currentBuild
    }

    static func markSeen() {
        UserDefaults.standard.set(currentBuild, forKey: seenKey)
    }
}

struct ChangelogView: View {
    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 22) {
                ForEach(Array(Changelog.releases.enumerated()), id: \.element.id) { index, release in
                    VStack(alignment: .leading, spacing: 14) {
                        HStack(alignment: .firstTextBaseline, spacing: 10) {
                            Text(release.version)
                                .font(.title2.weight(.bold).monospacedDigit())
                            Text(tr(release.title))
                                .font(.subheadline)
                                .foregroundStyle(.secondary)
                            Spacer(minLength: 0)
                            if index == 0 {
                                Text(tr("LATEST"))
                                    .font(.caption2.weight(.bold))
                                    .tracking(0.8)
                                    .foregroundStyle(.white)
                                    .padding(.horizontal, 9)
                                    .padding(.vertical, 4)
                                    .background(.tint, in: Capsule())
                            }
                        }

                        ForEach(release.changes) { change in
                            HStack(alignment: .top, spacing: 12) {
                                Image(systemName: change.icon)
                                    .font(.subheadline)
                                    .foregroundStyle(.tint)
                                    .frame(width: 24)
                                Text(tr(change.text))
                                    .font(.subheadline)
                                    .fixedSize(horizontal: false, vertical: true)
                                Spacer(minLength: 0)
                            }
                        }
                    }
                    .padding(18)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 24, style: .continuous))
                }
            }
            .padding(.horizontal, 20)
            .padding(.top, 8)
            .padding(.bottom, 32)
        }
        .background {
            AmbientBackground()
        }
        .navigationTitle(tr("What's New"))
        .navigationBarTitleDisplayMode(.inline)
    }
}

struct ChangelogSheet: View {
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        NavigationStack {
            ChangelogView()
                .toolbar {
                    ToolbarItem(placement: .confirmationAction) {
                        Button(tr("Done")) {
                            dismiss()
                        }
                    }
                }
        }
    }
}
