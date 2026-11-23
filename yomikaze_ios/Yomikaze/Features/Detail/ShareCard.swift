import SwiftUI
import UIKit

struct ShareCardImage: Identifiable {
    let id = UUID()
    let image: UIImage
    let title: String
}

// rendered off-screen into an image, so it only uses plain colours (no glass or materials)
struct ShareCardView: View {
    let title: String
    let authors: String
    let chapterCount: Int
    let status: String
    let type: String
    let year: String
    let tags: [String]
    let cover: UIImage?
    let backdrop: UIImage?
    let accent: Color

    static let size = CGSize(width: 360, height: 640)

    private var facts: [(value: String, label: String)] {
        var list: [(value: String, label: String)] = []
        if chapterCount > 0 {
            list.append(("\(chapterCount)", "CHAPTERS"))
        }
        if !status.isEmpty {
            list.append((status, "STATUS"))
        }
        if !year.isEmpty {
            list.append((year, "YEAR"))
        }
        return list
    }

    var body: some View {
        ZStack {
            background

            VStack(spacing: 0) {
                brandRow
                    .padding(.top, 22)
                    .padding(.horizontal, 24)

                Spacer(minLength: 18)

                coverStack

                Spacer(minLength: 22)

                Text(title)
                    .font(.system(size: 27, weight: .heavy))
                    .foregroundStyle(.white)
                    .multilineTextAlignment(.center)
                    .lineLimit(2)
                    .minimumScaleFactor(0.65)
                    .padding(.horizontal, 26)

                if !authors.isEmpty {
                    Text(authors)
                        .font(.system(size: 13, weight: .medium))
                        .foregroundStyle(.white.opacity(0.7))
                        .lineLimit(1)
                        .padding(.top, 5)
                        .padding(.horizontal, 26)
                }

                if !facts.isEmpty {
                    factPanel
                        .padding(.top, 16)
                        .padding(.horizontal, 24)
                }

                if !tags.isEmpty {
                    HStack(spacing: 6) {
                        ForEach(Array(tags.prefix(3)), id: \.self) { tag in
                            Text(tag)
                                .font(.system(size: 11, weight: .semibold))
                                .foregroundStyle(.white.opacity(0.9))
                                .lineLimit(1)
                                .padding(.horizontal, 11)
                                .padding(.vertical, 5)
                                .overlay {
                                    Capsule()
                                        .strokeBorder(Color.white.opacity(0.3), lineWidth: 1)
                                }
                        }
                    }
                    .padding(.top, 12)
                }

                Spacer(minLength: 16)

                VStack(spacing: 9) {
                    Capsule()
                        .fill(accent)
                        .frame(width: 44, height: 4)
                    Text("READ ON YOMIKAZE")
                        .font(.system(size: 10, weight: .bold))
                        .tracking(2.2)
                        .foregroundStyle(.white.opacity(0.6))
                }
                .padding(.bottom, 22)
            }
        }
        .frame(width: ShareCardView.size.width, height: ShareCardView.size.height)
        .clipShape(RoundedRectangle(cornerRadius: 34, style: .continuous))
    }

    private var background: some View {
        ZStack {
            Color(red: 0.06, green: 0.05, blue: 0.12)

            if let backdrop {
                Image(uiImage: backdrop)
                    .resizable()
                    .interpolation(.high)
                    .scaledToFill()
                    .frame(width: ShareCardView.size.width, height: ShareCardView.size.height)
                    .clipped()
                    .opacity(0.8)
            }

            RadialGradient(
                colors: [accent.opacity(0.45), accent.opacity(0)],
                center: UnitPoint(x: 0.5, y: 0.3),
                startRadius: 10,
                endRadius: 260
            )
            LinearGradient(
                stops: [
                    .init(color: Color.black.opacity(0.25), location: 0),
                    .init(color: Color.black.opacity(0.35), location: 0.45),
                    .init(color: Color.black.opacity(0.88), location: 1),
                ],
                startPoint: .top,
                endPoint: .bottom
            )
        }
    }

    private var brandRow: some View {
        HStack(spacing: 7) {
            Text("読")
                .font(.system(size: 17, weight: .bold))
                .foregroundStyle(accent)
            Text("YOMIKAZE")
                .font(.system(size: 11, weight: .bold))
                .tracking(2)
                .foregroundStyle(.white.opacity(0.85))
            Spacer()
            if !type.isEmpty {
                Text(type.uppercased())
                    .font(.system(size: 10, weight: .bold))
                    .tracking(1)
                    .foregroundStyle(.white)
                    .padding(.horizontal, 10)
                    .padding(.vertical, 5)
                    .background(accent, in: Capsule())
            }
        }
    }

    private var coverStack: some View {
        ZStack {
            RoundedRectangle(cornerRadius: 20, style: .continuous)
                .fill(Color.white.opacity(0.10))
                .frame(width: 196, height: 294)
                .rotationEffect(.degrees(-8))
                .offset(x: -14, y: 6)
            RoundedRectangle(cornerRadius: 20, style: .continuous)
                .fill(Color.white.opacity(0.16))
                .frame(width: 196, height: 294)
                .rotationEffect(.degrees(6))
                .offset(x: 12, y: 4)

            Group {
                if let cover {
                    Image(uiImage: cover)
                        .resizable()
                        .scaledToFill()
                } else {
                    Color.white.opacity(0.12)
                }
            }
            .frame(width: 196, height: 294)
            .clipShape(RoundedRectangle(cornerRadius: 20, style: .continuous))
            .overlay {
                RoundedRectangle(cornerRadius: 20, style: .continuous)
                    .strokeBorder(Color.white.opacity(0.22), lineWidth: 1)
            }
            .shadow(color: .black.opacity(0.55), radius: 22, y: 14)
        }
    }

    private var factPanel: some View {
        HStack(spacing: 0) {
            ForEach(Array(facts.enumerated()), id: \.offset) { index, fact in
                if index > 0 {
                    Rectangle()
                        .fill(Color.white.opacity(0.18))
                        .frame(width: 1, height: 26)
                }
                VStack(spacing: 3) {
                    Text(fact.value)
                        .font(.system(size: 16, weight: .bold))
                        .foregroundStyle(.white)
                        .lineLimit(1)
                        .minimumScaleFactor(0.6)
                    Text(fact.label)
                        .font(.system(size: 9, weight: .bold))
                        .tracking(1.2)
                        .foregroundStyle(.white.opacity(0.55))
                }
                .frame(maxWidth: .infinity)
                .padding(.horizontal, 4)
            }
        }
        .padding(.vertical, 12)
        .background(Color.white.opacity(0.10), in: RoundedRectangle(cornerRadius: 18, style: .continuous))
        .overlay {
            RoundedRectangle(cornerRadius: 18, style: .continuous)
                .strokeBorder(Color.white.opacity(0.14), lineWidth: 1)
        }
    }

    // renders the card 1080 px wide
    @MainActor
    func rendered() -> UIImage? {
        let renderer = ImageRenderer(content: self)
        renderer.scale = 3
        return renderer.uiImage
    }
}

struct ShareCardSheet: View {
    let card: ShareCardImage

    @Environment(\.dismiss) private var dismiss

    var body: some View {
        NavigationStack {
            VStack(spacing: 24) {
                Image(uiImage: card.image)
                    .resizable()
                    .scaledToFit()
                    .frame(maxHeight: .infinity)
                    .shadow(color: .black.opacity(0.3), radius: 20, y: 10)

                ShareLink(
                    item: Image(uiImage: card.image),
                    preview: SharePreview(card.title, image: Image(uiImage: card.image))
                ) {
                    Label(tr("Share image"), systemImage: "square.and.arrow.up")
                        .font(.headline)
                        .frame(maxWidth: .infinity)
                        .padding(.vertical, 6)
                }
                .buttonStyle(.glassProminent)
            }
            .padding(24)
            .navigationTitle(tr("Share card"))
            .navigationBarTitleDisplayMode(.inline)
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
