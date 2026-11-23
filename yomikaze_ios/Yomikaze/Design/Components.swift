import SwiftUI
import UIKit

struct RemoteImage: View {
    let url: URL?
    var maxWidth: CGFloat = 600

    @State private var image: UIImage?

    var body: some View {
        Rectangle()
            .fill(.quaternary)
            .overlay {
                if let image {
                    Image(uiImage: image)
                        .resizable()
                        .scaledToFill()
                        .allowsHitTesting(false)
                }
            }
            .clipped()
            .contentShape(Rectangle())
            .task(id: url) {
                await load()
            }
    }

    private func load() async {
        guard let url else {
            image = nil
            return
        }
        if let cached = ImageLoader.cached(url) {
            image = cached
            return
        }
        image = nil
        let loaded = await ImageLoader.image(for: url, maxWidth: maxWidth)
        guard !Task.isCancelled, let loaded else { return }
        image = loaded
    }
}

extension EnvironmentValues {
    // namespace for the zoom transition into the series page
    @Entry var zoomNamespace: Namespace.ID? = nil
}

// the zoom starts from the cover only, so titles and labels are not dragged along
struct ZoomSource: ViewModifier {
    let id: String
    let cornerRadius: CGFloat

    @Environment(\.zoomNamespace) private var namespace

    func body(content: Content) -> some View {
        if let namespace {
            content.matchedTransitionSource(id: id, in: namespace) { source in
                source.clipShape(RoundedRectangle(cornerRadius: cornerRadius, style: .continuous))
            }
        } else {
            content
        }
    }
}

extension View {
    func zoomSource(id: String, cornerRadius: CGFloat = 14) -> some View {
        modifier(ZoomSource(id: id, cornerRadius: cornerRadius))
    }
}

struct CoverView: View {
    let mangaID: String
    var cornerRadius: CGFloat = 14
    // 200x300 file, enough for list rows and cheaper to decode
    var small: Bool = false

    var body: some View {
        Color.clear
            .aspectRatio(2.0 / 3.0, contentMode: .fit)
            .overlay {
                RemoteImage(url: WeebCentral.coverURL(mangaID, small: small), maxWidth: small ? 200 : 600)
            }
            .clipShape(RoundedRectangle(cornerRadius: cornerRadius, style: .continuous))
            .overlay {
                RoundedRectangle(cornerRadius: cornerRadius, style: .continuous)
                    .strokeBorder(Color.primary.opacity(0.08), lineWidth: 1)
            }
    }
}

// the blur is baked into a tiny image once, a live .blur would be redone while scrolling
struct BlurredCover: View {
    let mangaID: String

    @State private var image: UIImage?

    var body: some View {
        Rectangle()
            .fill(.quaternary)
            .overlay {
                if let image {
                    Image(uiImage: image)
                        .resizable()
                        .interpolation(.high)
                        .scaledToFill()
                        .allowsHitTesting(false)
                }
            }
            .clipped()
            .contentShape(Rectangle())
            .task(id: mangaID) {
                let url = WeebCentral.coverURL(mangaID, small: true)
                if let cached = ImageLoader.cachedBlur(url) {
                    image = cached
                    return
                }
                let loaded = await ImageLoader.blurredImage(for: url)
                if !Task.isCancelled, let loaded {
                    image = loaded
                }
            }
    }
}

struct MangaCard: View {
    let manga: MangaSummary
    var subtitle: String? = nil

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            CoverView(mangaID: manga.id)
                .zoomSource(id: manga.id)
            Text(manga.title)
                .font(.footnote.weight(.semibold))
                .lineLimit(2, reservesSpace: true)
                .multilineTextAlignment(.leading)
            if let subtitle, !subtitle.isEmpty {
                Text(subtitle)
                    .font(.caption2)
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
            }
        }
    }
}

struct MangaRow: View {
    let manga: MangaSummary
    var detail: String? = nil

    var body: some View {
        HStack(spacing: 14) {
            CoverView(mangaID: manga.id, cornerRadius: 10, small: true)
                .zoomSource(id: manga.id, cornerRadius: 10)
                .frame(width: 60)
            VStack(alignment: .leading, spacing: 4) {
                Text(manga.title)
                    .font(.body.weight(.semibold))
                    .lineLimit(2)
                    .multilineTextAlignment(.leading)
                let line = detail ?? manga.metaLine
                if !line.isEmpty {
                    Text(line)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                }
            }
            Spacer(minLength: 0)
        }
        .contentShape(Rectangle())
    }
}

struct Chip: View {
    let text: String
    var systemImage: String? = nil

    var body: some View {
        HStack(spacing: 5) {
            if let systemImage {
                Image(systemName: systemImage)
            }
            Text(text)
        }
        .font(.caption.weight(.medium))
        .padding(.horizontal, 11)
        .padding(.vertical, 6)
        .background(.quaternary, in: Capsule())
    }
}

struct SectionHeader: View {
    let title: String
    var subtitle: String? = nil

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(title)
                .font(.title2.weight(.bold))
            if let subtitle {
                Text(subtitle)
                    .font(.footnote)
                    .foregroundStyle(.secondary)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}

struct ErrorStateView: View {
    let message: String
    let retry: () -> Void

    var body: some View {
        ContentUnavailableView {
            Label(tr("Couldn't load"), systemImage: "wifi.exclamationmark")
        } description: {
            Text(message)
        } actions: {
            Button(tr("Try again"), action: retry)
                .buttonStyle(.glassProminent)
        }
    }
}

struct OfflineBanner: View {
    var text: String = tr("You're offline. Downloaded chapters are still readable.")

    var body: some View {
        Label(text, systemImage: "wifi.slash")
            .font(.footnote.weight(.medium))
            .foregroundStyle(.secondary)
            .padding(.horizontal, 14)
            .padding(.vertical, 10)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(.quaternary, in: RoundedRectangle(cornerRadius: 14, style: .continuous))
    }
}

// own view so progress updates only redraw this control, not the whole list
struct DownloadControl: View {
    let chapterID: String
    let isDownloaded: Bool
    let start: () -> Void

    private let downloader = DownloadManager.shared

    var body: some View {
        if isDownloaded {
            Image(systemName: "arrow.down.circle.fill")
                .font(.title3)
                .foregroundStyle(.tint)
                .frame(width: 40, height: 40)
        } else if let fraction = downloader.progress[chapterID] {
            Button {
                downloader.cancel(chapterID)
            } label: {
                ZStack {
                    Circle()
                        .stroke(Color.secondary.opacity(0.25), lineWidth: 3)
                    Circle()
                        .trim(from: 0, to: max(0.03, fraction))
                        .stroke(.tint, style: StrokeStyle(lineWidth: 3, lineCap: .round))
                        .rotationEffect(.degrees(-90))
                    Image(systemName: "stop.fill")
                        .font(.system(size: 8))
                        .foregroundStyle(.tint)
                }
                .frame(width: 22, height: 22)
                .frame(width: 40, height: 40)
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
        } else if downloader.queued.contains(chapterID) {
            Button {
                downloader.cancel(chapterID)
            } label: {
                Image(systemName: "clock")
                    .font(.title3)
                    .foregroundStyle(.secondary)
                    .frame(width: 40, height: 40)
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
        } else {
            Button(action: start) {
                Image(systemName: downloader.failed.contains(chapterID) ? "exclamationmark.arrow.circlepath" : "arrow.down.circle")
                    .font(.title3)
                    .foregroundStyle(.secondary)
                    .frame(width: 40, height: 40)
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
        }
    }
}

extension Int64 {
    var formattedByteCount: String {
        ByteCountFormatter.string(fromByteCount: self, countStyle: .file)
    }
}
