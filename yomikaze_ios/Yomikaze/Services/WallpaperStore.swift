import Observation
import SwiftUI
import UIKit

@MainActor
@Observable
final class WallpaperStore {
    static let shared = WallpaperStore()

    private(set) var image: UIImage?

    private let fileURL: URL

    private init() {
        let support = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
        try? FileManager.default.createDirectory(at: support, withIntermediateDirectories: true)
        fileURL = support.appendingPathComponent("wallpaper.jpg")
        if let data = try? Data(contentsOf: fileURL) {
            image = UIImage(data: data)
        }
    }

    // scaled down and saved off the main thread, returns a matching accent
    @discardableResult
    func set(_ data: Data) async -> Color? {
        let target = fileURL
        let result = await Task.detached(priority: .userInitiated) { () -> (UIImage, Color?)? in
            // big enough to stay sharp on the largest iphone and ipad screens
            guard let scaled = ImageDecoder.decode(data, maxWidth: 2800) else { return nil }
            if let jpeg = scaled.jpegData(compressionQuality: 0.85) {
                try? jpeg.write(to: target, options: .atomic)
            }
            return (scaled, WallpaperStore.accent(matching: scaled))
        }.value
        guard let result else { return nil }
        image = result.0
        return result.1
    }

    func clear() {
        try? FileManager.default.removeItem(at: fileURL)
        image = nil
    }

    var matchingAccent: Color? {
        guard let image else { return nil }
        return WallpaperStore.accent(matching: image)
    }

    // average colour pushed towards something vivid enough for an accent
    nonisolated static func accent(matching image: UIImage) -> Color? {
        guard let source = image.cgImage else { return nil }
        var pixel = [UInt8](repeating: 0, count: 4)
        guard let context = CGContext(
            data: &pixel,
            width: 1,
            height: 1,
            bitsPerComponent: 8,
            bytesPerRow: 4,
            space: CGColorSpaceCreateDeviceRGB(),
            bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
        ) else { return nil }
        context.interpolationQuality = .medium
        context.draw(source, in: CGRect(x: 0, y: 0, width: 1, height: 1))

        let average = UIColor(
            red: CGFloat(pixel[0]) / 255,
            green: CGFloat(pixel[1]) / 255,
            blue: CGFloat(pixel[2]) / 255,
            alpha: 1
        )
        var hue: CGFloat = 0
        var saturation: CGFloat = 0
        var brightness: CGFloat = 0
        var alpha: CGFloat = 0
        guard average.getHue(&hue, saturation: &saturation, brightness: &brightness, alpha: &alpha) else {
            return Color(average)
        }
        return Color(
            hue: Double(hue),
            saturation: Double(min(max(saturation * 1.6, 0.5), 0.85)),
            brightness: Double(min(max(brightness * 1.3, 0.75), 0.98))
        )
    }
}
