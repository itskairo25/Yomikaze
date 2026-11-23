import SwiftUI
import UIKit

enum ReaderBackground: String, CaseIterable, Identifiable {
    case black
    case gray
    case sepia
    case white

    var id: String { rawValue }

    var name: String {
        switch self {
        case .black: return tr("Black")
        case .gray: return tr("Grey")
        case .sepia: return tr("Sepia")
        case .white: return tr("White")
        }
    }

    var color: Color {
        switch self {
        case .black: return .black
        case .gray: return Color(white: 0.14)
        case .sepia: return Color(red: 0.93, green: 0.87, blue: 0.76)
        case .white: return .white
        }
    }
}

// shown as a sheet in the reader and as a section in settings
struct ReaderOptions: View {
    @AppStorage(SettingsKey.readerMode) private var modeRaw = ReaderMode.vertical.rawValue
    @AppStorage(SettingsKey.tapZones) private var tapZones = true
    @AppStorage(SettingsKey.readerBackground) private var backgroundRaw = ReaderBackground.black.rawValue
    @AppStorage(SettingsKey.readerDim) private var dim = 0.0
    @AppStorage(SettingsKey.readerGap) private var pageGap = false
    @AppStorage(SettingsKey.keepAwake) private var keepAwake = true
    @AppStorage(SettingsKey.readerTwoPages) private var twoPages = "auto"
    @AppStorage(SettingsKey.readerProgressBar) private var progressBar = true
    @AppStorage(SettingsKey.readerWidth) private var readerWidth = 900.0

    var body: some View {
        Picker(tr("Reading mode"), selection: $modeRaw) {
            ForEach(ReaderMode.allCases) { mode in
                Label(mode.title, systemImage: mode.icon).tag(mode.rawValue)
            }
        }

        VStack(alignment: .leading, spacing: 4) {
            Picker(tr("Two pages"), selection: $twoPages) {
                Text(tr("Auto")).tag("auto")
                Text(tr("On")).tag("on")
                Text(tr("Off")).tag("off")
            }
            Text(tr("In paged mode two pages sit side by side, by themselves on a wide screen like an iPad in landscape."))
                .font(.caption)
                .foregroundStyle(.secondary)
        }

        Toggle(isOn: $tapZones) {
            VStack(alignment: .leading, spacing: 2) {
                Text(tr("Tap zones"))
                Text(tr("Tap the left or right edge to turn the page."))
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
        }

        Picker(tr("Background"), selection: $backgroundRaw) {
            ForEach(ReaderBackground.allCases) { option in
                Text(option.name).tag(option.rawValue)
            }
        }

        VStack(alignment: .leading, spacing: 4) {
            Text(tr("Dim pages"))
            HStack(spacing: 10) {
                Image(systemName: "sun.max.fill")
                    .foregroundStyle(.secondary)
                Slider(value: $dim, in: 0...0.7)
                Image(systemName: "moon.fill")
                    .foregroundStyle(.secondary)
            }
        }

        if UIDevice.current.userInterfaceIdiom == .pad {
            VStack(alignment: .leading, spacing: 4) {
                Text(tr("Page width"))
                HStack(spacing: 10) {
                    Image(systemName: "rectangle.portrait")
                        .foregroundStyle(.secondary)
                    Slider(value: $readerWidth, in: 500...1400)
                    Image(systemName: "rectangle")
                        .foregroundStyle(.secondary)
                }
                Text(tr("How wide pages get in vertical mode."))
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
        }

        Toggle(tr("Progress line at the bottom"), isOn: $progressBar)
        Toggle(tr("Gap between pages (vertical mode)"), isOn: $pageGap)
        Toggle(tr("Keep screen awake"), isOn: $keepAwake)
    }
}

struct ReaderSettingsSheet: View {
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        NavigationStack {
            Form {
                ReaderOptions()
            }
            .navigationTitle(tr("Reader"))
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .confirmationAction) {
                    Button(tr("Done")) {
                        dismiss()
                    }
                }
            }
        }
        .presentationDetents([.medium, .large])
    }
}
