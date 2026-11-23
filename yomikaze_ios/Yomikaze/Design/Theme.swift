import SwiftUI
import UIKit

enum SettingsKey {
    static let accent = "accent"
    static let appearance = "appearance"
    static let readerMode = "readerMode"
    static let keepAwake = "keepAwake"
    static let showAdult = "showAdult"
    static let librarySort = "librarySort"
    static let recentSearches = "recentSearches"
    static let offlineMode = OfflineMode.key
    static let customAccent = "customAccent"
    static let wallpaperDim = "wallpaperDim"
    static let wallpaperZoom = "wallpaperZoom"
    static let wallpaperX = "wallpaperX"
    static let wallpaperY = "wallpaperY"
    // link of the gallery picture in use, empty for one picked from photos
    static let wallpaperSource = "wallpaperSource"
    static let tapZones = "tapZones"
    static let readerBackground = "readerBackground"
    static let readerDim = "readerDim"
    static let readerGap = "readerGap"
    static let readerTwoPages = "readerTwoPages"
    static let readerProgressBar = "readerProgressBar"
    // widest a page gets in vertical mode on an ipad
    static let readerWidth = "readerWidth"
    static let appLock = "appLock"
    static let libraryCategories = "libraryCategories"
}

enum AccentChoice: String, CaseIterable, Identifiable {
    case yoru
    case kaze
    case sakura
    case akane
    case kohaku

    var id: String { rawValue }

    var name: String {
        switch self {
        case .yoru: return "Yoru"
        case .kaze: return "Kaze"
        case .sakura: return "Sakura"
        case .akane: return "Akane"
        case .kohaku: return "Kohaku"
        }
    }

    var color: Color {
        switch self {
        case .yoru: return Color(red: 0.49, green: 0.45, blue: 1.00)
        case .kaze: return Color(red: 0.16, green: 0.74, blue: 0.78)
        case .sakura: return Color(red: 1.00, green: 0.42, blue: 0.64)
        case .akane: return Color(red: 1.00, green: 0.38, blue: 0.30)
        case .kohaku: return Color(red: 0.98, green: 0.66, blue: 0.16)
        }
    }

    static func current(_ rawValue: String) -> AccentChoice {
        AccentChoice(rawValue: rawValue) ?? .yoru
    }
}

enum AppearanceChoice: String, CaseIterable, Identifiable {
    case system
    case light
    case dark

    var id: String { rawValue }

    var name: String {
        switch self {
        case .system: return tr("System")
        case .light: return tr("Light")
        case .dark: return tr("Dark")
        }
    }

    var colorScheme: ColorScheme? {
        switch self {
        case .system: return nil
        case .light: return .light
        case .dark: return .dark
        }
    }
}

enum ReaderMode: String, CaseIterable, Identifiable {
    case vertical
    case pagedLTR
    case pagedRTL

    var id: String { rawValue }

    var title: String {
        switch self {
        case .vertical: return tr("Vertical scroll")
        case .pagedLTR: return tr("Paged, left to right")
        case .pagedRTL: return tr("Paged, right to left")
        }
    }

    var icon: String {
        switch self {
        case .vertical: return "arrow.up.and.down"
        case .pagedLTR: return "arrow.right"
        case .pagedRTL: return "arrow.left"
        }
    }
}

// accent glow behind the main screens, gives the glass bars something to refract
struct AmbientBackground: View {
    @AppStorage(SettingsKey.accent) private var accentRaw = AccentChoice.yoru.rawValue
    @AppStorage(SettingsKey.customAccent) private var customAccentHex = AppTheme.defaultCustomAccent
    @AppStorage(SettingsKey.wallpaperDim) private var wallpaperDim = 0.6
    @AppStorage(SettingsKey.wallpaperZoom) private var wallpaperZoom = 1.0
    @AppStorage(SettingsKey.wallpaperX) private var wallpaperX = 0.0
    @AppStorage(SettingsKey.wallpaperY) private var wallpaperY = 0.0
    @Environment(\.colorScheme) private var colorScheme

    private let wallpaper = WallpaperStore.shared

    var body: some View {
        let accent = AppTheme.accent(raw: accentRaw, customHex: customAccentHex)
        ZStack(alignment: .top) {
            Color(uiColor: .systemBackground)

            if let image = wallpaper.image {
                // the user's picture, toned down so text and covers stay readable
                WallpaperImage(image: image, zoom: wallpaperZoom, offsetX: wallpaperX, offsetY: wallpaperY)
                Color(uiColor: .systemBackground)
                    .opacity(wallpaperDim)
            } else {
                LinearGradient(
                    colors: [accent.opacity(colorScheme == .dark ? 0.30 : 0.20), accent.opacity(0)],
                    startPoint: .top,
                    endPoint: .bottom
                )
                .frame(height: 440)
            }
        }
        .ignoresSafeArea()
    }
}

// the wallpaper always covers the container, zoom enlarges it and the offsets (-1 to 1) move it within the overflow
struct WallpaperLayout {
    let size: CGSize
    let overflow: CGSize

    init(image: CGSize, container: CGSize, zoom: Double) {
        guard image.width > 0, image.height > 0, container.width > 0, container.height > 0 else {
            size = container
            overflow = .zero
            return
        }
        let fill = max(container.width / image.width, container.height / image.height) * CGFloat(max(zoom, 1))
        size = CGSize(width: image.width * fill, height: image.height * fill)
        overflow = CGSize(width: max(size.width - container.width, 0), height: max(size.height - container.height, 0))
    }
}

struct WallpaperImage: View {
    let image: UIImage
    let zoom: Double
    let offsetX: Double
    let offsetY: Double

    var body: some View {
        GeometryReader { proxy in
            let layout = WallpaperLayout(image: image.size, container: proxy.size, zoom: zoom)
            Image(uiImage: image)
                .resizable()
                .frame(width: layout.size.width, height: layout.size.height)
                .position(
                    x: proxy.size.width / 2 + CGFloat(offsetX) * layout.overflow.width / 2,
                    y: proxy.size.height / 2 + CGFloat(offsetY) * layout.overflow.height / 2
                )
                .allowsHitTesting(false)
        }
        .clipped()
    }
}

enum AppTheme {
    static let customAccentID = "custom"
    static let defaultCustomAccent = "#7D73FF"

    static func accent(raw: String, customHex: String) -> Color {
        if raw == customAccentID {
            return Color(hex: customHex) ?? AccentChoice.yoru.color
        }
        return AccentChoice.current(raw).color
    }
}

extension Color {
    init?(hex: String) {
        let cleaned = hex.trimmingCharacters(in: CharacterSet(charactersIn: "# "))
        guard cleaned.count == 6, let value = UInt32(cleaned, radix: 16) else { return nil }
        self.init(
            red: Double((value >> 16) & 0xFF) / 255,
            green: Double((value >> 8) & 0xFF) / 255,
            blue: Double(value & 0xFF) / 255
        )
    }

    var hexString: String {
        var red: CGFloat = 0
        var green: CGFloat = 0
        var blue: CGFloat = 0
        var alpha: CGFloat = 0
        UIColor(self).getRed(&red, green: &green, blue: &blue, alpha: &alpha)
        let clamp: (CGFloat) -> Int = { Int((min(max($0, 0), 1) * 255).rounded()) }
        return String(format: "#%02X%02X%02X", clamp(red), clamp(green), clamp(blue))
    }
}
