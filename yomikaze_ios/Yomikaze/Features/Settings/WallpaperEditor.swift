import SwiftUI

struct WallpaperEditor: View {
    let image: UIImage

    @Environment(\.dismiss) private var dismiss
    @AppStorage(SettingsKey.wallpaperZoom) private var zoom = 1.0
    @AppStorage(SettingsKey.wallpaperX) private var offsetX = 0.0
    @AppStorage(SettingsKey.wallpaperY) private var offsetY = 0.0

    @State private var dragStart: CGPoint?
    @State private var zoomStart: Double?

    var body: some View {
        NavigationStack {
            VStack(spacing: 20) {
                Text(tr("Drag to move, pinch to zoom"))
                    .font(.footnote)
                    .foregroundStyle(.secondary)

                GeometryReader { proxy in
                    WallpaperImage(image: image, zoom: zoom, offsetX: offsetX, offsetY: offsetY)
                        .contentShape(Rectangle())
                        .gesture(dragGesture(container: proxy.size))
                        .simultaneousGesture(zoomGesture)
                }
                .aspectRatio(9.0 / 19.5, contentMode: .fit)
                .clipShape(RoundedRectangle(cornerRadius: 28, style: .continuous))
                .overlay {
                    RoundedRectangle(cornerRadius: 28, style: .continuous)
                        .strokeBorder(Color.primary.opacity(0.2), lineWidth: 1)
                }
                .frame(maxHeight: .infinity)

                HStack(spacing: 12) {
                    Image(systemName: "minus.magnifyingglass")
                        .foregroundStyle(.secondary)
                    Slider(value: $zoom, in: 1...4)
                    Image(systemName: "plus.magnifyingglass")
                        .foregroundStyle(.secondary)
                }

                Button(tr("Reset position")) {
                    withAnimation(.snappy) {
                        zoom = 1
                        offsetX = 0
                        offsetY = 0
                    }
                }
                .buttonStyle(.glass)
            }
            .padding(20)
            .navigationTitle(tr("Adjust background"))
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

    private func dragGesture(container: CGSize) -> some Gesture {
        DragGesture()
            .onChanged { value in
                let start = dragStart ?? CGPoint(x: offsetX, y: offsetY)
                dragStart = start
                let layout = WallpaperLayout(image: image.size, container: container, zoom: zoom)
                // offsets are a fraction of the overflow so they work on any screen size
                if layout.overflow.width > 0 {
                    offsetX = clamp(Double(start.x) + Double(value.translation.width / (layout.overflow.width / 2)))
                }
                if layout.overflow.height > 0 {
                    offsetY = clamp(Double(start.y) + Double(value.translation.height / (layout.overflow.height / 2)))
                }
            }
            .onEnded { _ in
                dragStart = nil
            }
    }

    private var zoomGesture: some Gesture {
        MagnifyGesture()
            .onChanged { value in
                let start = zoomStart ?? zoom
                zoomStart = start
                zoom = min(max(start * Double(value.magnification), 1), 4)
            }
            .onEnded { _ in
                zoomStart = nil
            }
    }

    private func clamp(_ value: Double) -> Double {
        min(max(value, -1), 1)
    }
}
