import CoreImage
import CryptoKit
import Foundation
import ImageIO
import UIKit

// two folders: Caches/Images for viewed images (ios may purge it)
// and Application Support/Downloads for downloaded chapters (kept until deleted)
actor ImageStore {
    static let shared = ImageStore()

    private let cacheDirectory: URL
    private let downloadsDirectory: URL
    private let session: URLSession
    private var inFlight: [URL: Task<Data, Error>] = [:]

    init() {
        let fileManager = FileManager.default
        let caches = fileManager.urls(for: .cachesDirectory, in: .userDomainMask)[0]
        let support = fileManager.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
        let cache = caches.appendingPathComponent("Images", isDirectory: true)
        var downloads = support.appendingPathComponent("Downloads", isDirectory: true)

        try? fileManager.createDirectory(at: cache, withIntermediateDirectories: true)
        try? fileManager.createDirectory(at: downloads, withIntermediateDirectories: true)

        // downloads can be fetched again, keep them out of icloud backups
        var values = URLResourceValues()
        values.isExcludedFromBackup = true
        try? downloads.setResourceValues(values)

        cacheDirectory = cache
        downloadsDirectory = downloads

        let configuration = URLSessionConfiguration.default
        configuration.urlCache = nil
        configuration.httpMaximumConnectionsPerHost = 6
        configuration.timeoutIntervalForRequest = 30
        configuration.httpAdditionalHeaders = [
            "User-Agent": WeebCentral.userAgent,
            "Referer": WeebCentral.base + "/",
        ]
        session = URLSession(configuration: configuration)
    }

    private static func fileName(for url: URL) -> String {
        let digest = SHA256.hash(data: Data(url.absoluteString.utf8))
        return digest.map { String(format: "%02x", $0) }.joined()
    }

    // keep stores the file in the permanent downloads folder
    func data(for url: URL, keep: Bool = false) async throws -> Data {
        let name = ImageStore.fileName(for: url)
        let downloadedFile = downloadsDirectory.appendingPathComponent(name)
        let cachedFile = cacheDirectory.appendingPathComponent(name)

        if let data = try? Data(contentsOf: downloadedFile) {
            return data
        }
        if let data = try? Data(contentsOf: cachedFile) {
            if keep {
                try? data.write(to: downloadedFile, options: .atomic)
            }
            return data
        }

        // offline mode only shows what is on the device
        if OfflineMode.isForced {
            throw URLError(.notConnectedToInternet)
        }

        let task: Task<Data, Error>
        if let running = inFlight[url] {
            task = running
        } else {
            let session = self.session
            task = Task {
                var lastError: Error = URLError(.unknown)
                for attempt in 0..<2 {
                    do {
                        let (data, response) = try await session.data(from: url)
                        guard let http = response as? HTTPURLResponse,
                              (200..<300).contains(http.statusCode),
                              !data.isEmpty
                        else {
                            throw URLError(.badServerResponse)
                        }
                        return data
                    } catch {
                        lastError = error
                        if attempt == 0 {
                            try? await Task.sleep(nanoseconds: 400_000_000)
                        }
                    }
                }
                throw lastError
            }
            inFlight[url] = task
        }

        do {
            let data = try await task.value
            inFlight[url] = nil
            try? data.write(to: keep ? downloadedFile : cachedFile, options: .atomic)
            return data
        } catch {
            inFlight[url] = nil
            throw error
        }
    }

    func removeDownloads(for urls: [URL]) {
        for url in urls {
            let file = downloadsDirectory.appendingPathComponent(ImageStore.fileName(for: url))
            try? FileManager.default.removeItem(at: file)
        }
    }

    func removeAllDownloads() {
        ImageStore.emptyDirectory(downloadsDirectory)
    }

    func clearCache() {
        ImageStore.emptyDirectory(cacheDirectory)
    }

    func cacheSize() -> Int64 {
        ImageStore.directorySize(cacheDirectory)
    }

    func downloadsSize() -> Int64 {
        ImageStore.directorySize(downloadsDirectory)
    }

    private static func emptyDirectory(_ directory: URL) {
        let fileManager = FileManager.default
        let files = (try? fileManager.contentsOfDirectory(at: directory, includingPropertiesForKeys: nil)) ?? []
        for file in files {
            try? fileManager.removeItem(at: file)
        }
    }

    private static func directorySize(_ directory: URL) -> Int64 {
        let fileManager = FileManager.default
        let files = (try? fileManager.contentsOfDirectory(at: directory, includingPropertiesForKeys: [.fileSizeKey])) ?? []
        var total: Int64 = 0
        for file in files {
            let size = (try? file.resourceValues(forKeys: [.fileSizeKey]))?.fileSize ?? 0
            total += Int64(size)
        }
        return total
    }
}

enum ImageDecoder {
    // scales down to maxWidth, tall webtoon strips keep their full height
    static func decode(_ data: Data, maxWidth: CGFloat) -> UIImage? {
        let sourceOptions: [CFString: Any] = [kCGImageSourceShouldCache: false]
        guard let source = CGImageSourceCreateWithData(data as CFData, sourceOptions as CFDictionary) else {
            return UIImage(data: data)
        }

        var maxPixelSize = maxWidth
        if let properties = CGImageSourceCopyPropertiesAtIndex(source, 0, nil) as? [CFString: Any],
           let width = (properties[kCGImagePropertyPixelWidth] as? NSNumber)?.doubleValue,
           let height = (properties[kCGImagePropertyPixelHeight] as? NSNumber)?.doubleValue,
           width > 0, height > 0 {
            let scale = min(1.0, Double(maxWidth) / width)
            maxPixelSize = CGFloat(max(width, height) * scale)
        }

        let thumbnailOptions: [CFString: Any] = [
            kCGImageSourceCreateThumbnailFromImageAlways: true,
            kCGImageSourceCreateThumbnailWithTransform: true,
            kCGImageSourceShouldCacheImmediately: true,
            kCGImageSourceThumbnailMaxPixelSize: maxPixelSize,
        ]
        guard let image = CGImageSourceCreateThumbnailAtIndex(source, 0, thumbnailOptions as CFDictionary) else {
            return UIImage(data: data)
        }
        return UIImage(cgImage: image)
    }
}

enum ImageLoader {
    private static let memory: NSCache<NSURL, UIImage> = {
        let cache = NSCache<NSURL, UIImage>()
        cache.totalCostLimit = 192 * 1024 * 1024
        return cache
    }()

    static func cached(_ url: URL) -> UIImage? {
        memory.object(forKey: url as NSURL)
    }

    static func image(for url: URL, maxWidth: CGFloat) async -> UIImage? {
        if let hit = cached(url) {
            return hit
        }
        do {
            let data = try await ImageStore.shared.data(for: url)
            if Task.isCancelled {
                return nil
            }
            let decoded = await Task.detached(priority: .userInitiated) {
                ImageDecoder.decode(data, maxWidth: maxWidth)
            }.value
            guard let image = decoded else { return nil }
            let cost = Int(image.size.width * image.scale * image.size.height * image.scale * 4)
            memory.setObject(image, forKey: url as NSURL, cost: cost)
            return image
        } catch {
            return nil
        }
    }

    static func clearMemory() {
        memory.removeAllObjects()
        blurMemory.removeAllObjects()
    }

    private static let blurMemory = NSCache<NSURL, UIImage>()
    private static let blurContext = CIContext()

    static func cachedBlur(_ url: URL) -> UIImage? {
        blurMemory.object(forKey: url as NSURL)
    }

    // blurring once is cheaper than a live blur the gpu redoes while scrolling
    static func blurredImage(for url: URL) async -> UIImage? {
        if let hit = cachedBlur(url) {
            return hit
        }
        guard let data = try? await ImageStore.shared.data(for: url) else {
            return nil
        }
        if Task.isCancelled {
            return nil
        }
        let context = blurContext
        let result = await Task.detached(priority: .utility) { () -> UIImage? in
            guard let small = ImageDecoder.decode(data, maxWidth: 64), let source = small.cgImage else {
                return nil
            }
            let input = CIImage(cgImage: source)
            let output = input
                .clampedToExtent()
                .applyingGaussianBlur(sigma: 6)
                .cropped(to: input.extent)
            guard let blurred = context.createCGImage(output, from: input.extent) else {
                return small
            }
            return UIImage(cgImage: blurred)
        }.value
        if let result {
            blurMemory.setObject(result, forKey: url as NSURL)
        }
        return result
    }
}
