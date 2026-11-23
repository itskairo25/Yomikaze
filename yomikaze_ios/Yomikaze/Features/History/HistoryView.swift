import SwiftData
import SwiftUI

struct HistoryView: View {
    @Environment(\.modelContext) private var context
    @Environment(AppRouter.self) private var router

    @Query(filter: #Predicate<SavedManga> { $0.hasHistory }, sort: \SavedManga.lastReadAt, order: .reverse)
    private var history: [SavedManga]

    @State private var confirmClear = false

    var body: some View {
        Group {
            if history.isEmpty {
                ContentUnavailableView {
                    Label(tr("Nothing read yet"), systemImage: "clock")
                } description: {
                    Text(tr("Chapters you open show up here, so you can pick up where you left off."))
                }
            } else {
                list
            }
        }
        .background {
            AmbientBackground()
        }
        .navigationTitle(tr("History"))
        .toolbar {
            ToolbarItem(placement: .topBarLeading) {
                NavigationLink {
                    StatsView()
                } label: {
                    Label(tr("Statistics"), systemImage: "chart.bar.fill")
                }
            }
            if !history.isEmpty {
                ToolbarItem(placement: .topBarTrailing) {
                    Button(role: .destructive) {
                        confirmClear = true
                    } label: {
                        Image(systemName: "trash")
                    }
                }
            }
        }
        .confirmationDialog(tr("Clear all reading history?"), isPresented: $confirmClear, titleVisibility: .visible) {
            Button(tr("Clear history"), role: .destructive) {
                for item in history {
                    context.removeFromHistory(item)
                }
                try? context.save()
            }
        } message: {
            Text(tr("Reading positions and read marks are removed. Your library and downloads stay."))
        }
        .navigationDestination(for: MangaSummary.self) { manga in
            MangaDetailView(manga: manga)
        }
    }

    private var list: some View {
        List {
            ForEach(history) { item in
                HStack(spacing: 4) {
                    NavigationLink(value: item.summary) {
                        MangaRow(manga: item.summary, detail: detailLine(item))
                    }
                    Button {
                        router.resume(item)
                    } label: {
                        // a glass effect in every row makes scrolling stutter
                        Image(systemName: "play.fill")
                            .font(.subheadline)
                            .foregroundStyle(.tint)
                            .frame(width: 36, height: 36)
                            .background(.quaternary, in: Circle())
                    }
                    .buttonStyle(.borderless)
                }
                .listRowBackground(Color.clear)
                .swipeActions(edge: .trailing) {
                    Button(role: .destructive) {
                        context.removeFromHistory(item)
                        try? context.save()
                    } label: {
                        Label(tr("Remove"), systemImage: "trash")
                    }
                }
            }
        }
        .listStyle(.plain)
        .scrollContentBackground(.hidden)
    }

    private func detailLine(_ item: SavedManga) -> String {
        let when = item.lastReadAt.formatted(.relative(presentation: .named))
        if item.lastPageCount > 0 {
            return "\(item.lastChapterName) · \(item.lastPage + 1)/\(item.lastPageCount) · \(when)"
        }
        return "\(item.lastChapterName) · \(when)"
    }
}
