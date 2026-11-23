import SwiftData
import SwiftUI

struct StatsView: View {
    @Environment(AppRouter.self) private var router

    @Query(filter: #Predicate<SavedManga> { $0.hasHistory }, sort: \SavedManga.lastReadAt, order: .reverse)
    private var history: [SavedManga]
    @Query(filter: #Predicate<SavedManga> { $0.inLibrary }) private var library: [SavedManga]
    @Query(sort: \ChapterProgress.updatedAt, order: .reverse) private var progress: [ChapterProgress]
    @Query private var downloads: [DownloadedChapter]

    // chapter count per series, from the saved chapter lists
    @State private var totals: [String: Int] = [:]

    private let tileColumns = [GridItem(.flexible(), spacing: 12), GridItem(.flexible(), spacing: 12)]

    var body: some View {
        let snapshot = StatsSnapshot(progress: progress)

        ScrollView {
            if history.isEmpty {
                ContentUnavailableView {
                    Label(tr("No statistics yet"), systemImage: "chart.bar")
                } description: {
                    Text(tr("Read a chapter and your numbers show up here."))
                }
                .padding(.top, 80)
            } else {
                VStack(alignment: .leading, spacing: 26) {
                    if let last = history.first {
                        lastReadCard(last)
                    }

                    LazyVGrid(columns: tileColumns, spacing: 12) {
                        StatTile(value: "\(snapshot.chaptersRead)", label: tr("Chapters read"), icon: "checkmark.circle.fill")
                        StatTile(value: "\(snapshot.pagesRead)", label: tr("Pages read"), icon: "doc.text.fill")
                        StatTile(value: "\(history.count)", label: tr("Series started"), icon: "book.fill")
                        StatTile(value: "\(snapshot.streak)", label: snapshot.streak == 1 ? tr("Day streak") : tr("Days streak"), icon: "flame.fill")
                        StatTile(value: "\(library.count)", label: tr("In library"), icon: "bookmark.fill")
                        StatTile(value: "\(downloads.count)", label: tr("Downloaded chapters"), icon: "arrow.down.circle.fill")
                    }

                    weekSection(snapshot)

                    if !snapshot.inProgress.isEmpty {
                        inProgressSection(snapshot)
                    }

                    seriesSection(snapshot)
                }
                .padding(.horizontal, 20)
                .padding(.top, 8)
                .padding(.bottom, 32)
            }
        }
        .background {
            AmbientBackground()
        }
        .navigationTitle(tr("Statistics"))
        .task(id: history.count) {
            let ids = history.map { $0.mangaID }
            totals = await Task.detached(priority: .utility) {
                var result: [String: Int] = [:]
                for id in ids {
                    result[id] = (DiskCache.load([Chapter].self, name: DiskCache.chaptersName(id)) ?? []).count
                }
                return result
            }.value
        }
    }

    private func lastReadCard(_ manga: SavedManga) -> some View {
        let fraction = manga.lastPageCount > 0 ? Double(manga.lastPage + 1) / Double(manga.lastPageCount) : 0

        return VStack(alignment: .leading, spacing: 14) {
            Text(tr("LAST READ"))
                .font(.caption2.weight(.bold))
                .tracking(1.2)
                .foregroundStyle(.secondary)

            HStack(spacing: 14) {
                CoverView(mangaID: manga.mangaID, cornerRadius: 12, small: true)
                    .frame(width: 72)

                VStack(alignment: .leading, spacing: 6) {
                    Text(manga.title)
                        .font(.headline)
                        .lineLimit(2)
                    Text(manga.lastChapterName)
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                    if manga.lastPageCount > 0 {
                        ProgressBar(fraction: fraction)
                        Text(tr("Page {page} of {count} · {percent}%", ["page": manga.lastPage + 1, "count": manga.lastPageCount, "percent": Int((fraction * 100).rounded())]))
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                    Text(manga.lastReadAt.formatted(.relative(presentation: .named)))
                        .font(.caption2)
                        .foregroundStyle(.tertiary)
                }
                Spacer(minLength: 0)
            }

            Button {
                router.resume(manga)
            } label: {
                Label(tr("Continue reading"), systemImage: "play.fill")
                    .font(.headline)
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, 4)
            }
            .buttonStyle(.glassProminent)
        }
        .padding(18)
        .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 24, style: .continuous))
    }

    private func weekSection(_ snapshot: StatsSnapshot) -> some View {
        let peak = max(snapshot.week.map { $0.count }.max() ?? 0, 1)

        return VStack(alignment: .leading, spacing: 12) {
            Text(tr("Last 7 days"))
                .font(.title3.weight(.bold))

            HStack(alignment: .bottom, spacing: 10) {
                ForEach(snapshot.week) { day in
                    VStack(spacing: 6) {
                        Text(day.count > 0 ? "\(day.count)" : " ")
                            .font(.caption2.weight(.semibold))
                            .foregroundStyle(.secondary)
                        RoundedRectangle(cornerRadius: 6, style: .continuous)
                            .fill(day.count > 0 ? AnyShapeStyle(.tint) : AnyShapeStyle(.quaternary))
                            .frame(height: max(6, 96 * CGFloat(day.count) / CGFloat(peak)))
                        Text(day.label)
                            .font(.caption2)
                            .foregroundStyle(.secondary)
                    }
                    .frame(maxWidth: .infinity)
                }
            }
            .frame(height: 140, alignment: .bottom)
            .padding(16)
            .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 24, style: .continuous))
        }
    }

    private func inProgressSection(_ snapshot: StatsSnapshot) -> some View {
        let titles = Dictionary(history.map { ($0.mangaID, $0.title) }, uniquingKeysWith: { first, _ in first })

        return VStack(alignment: .leading, spacing: 12) {
            Text(tr("Chapters in progress"))
                .font(.title3.weight(.bold))

            VStack(spacing: 14) {
                ForEach(snapshot.inProgress.prefix(8)) { item in
                    let fraction = Double(item.page + 1) / Double(max(item.pageCount, 1))
                    VStack(alignment: .leading, spacing: 6) {
                        HStack(alignment: .firstTextBaseline) {
                            VStack(alignment: .leading, spacing: 1) {
                                Text(item.chapterName)
                                    .font(.subheadline.weight(.semibold))
                                Text(titles[item.mangaID] ?? "")
                                    .font(.caption)
                                    .foregroundStyle(.secondary)
                                    .lineLimit(1)
                            }
                            Spacer()
                            Text("\(Int((fraction * 100).rounded()))%")
                                .font(.subheadline.weight(.semibold).monospacedDigit())
                                .foregroundStyle(.tint)
                        }
                        ProgressBar(fraction: fraction)
                    }
                }
            }
            .padding(16)
            .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 24, style: .continuous))
        }
    }

    private func seriesSection(_ snapshot: StatsSnapshot) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            Text(tr("Progress by series"))
                .font(.title3.weight(.bold))

            VStack(spacing: 16) {
                ForEach(history) { manga in
                    let read = snapshot.readBySeries[manga.mangaID] ?? 0
                    let total = max(totals[manga.mangaID] ?? 0, read)
                    let fraction = total > 0 ? Double(read) / Double(total) : 0

                    NavigationLink(value: manga.summary) {
                        HStack(spacing: 12) {
                            CoverView(mangaID: manga.mangaID, cornerRadius: 8, small: true)
                                .frame(width: 44)
                            VStack(alignment: .leading, spacing: 6) {
                                HStack(alignment: .firstTextBaseline) {
                                    Text(manga.title)
                                        .font(.subheadline.weight(.semibold))
                                        .lineLimit(1)
                                    Spacer()
                                    Text("\(Int((fraction * 100).rounded()))%")
                                        .font(.subheadline.weight(.semibold).monospacedDigit())
                                        .foregroundStyle(.tint)
                                }
                                ProgressBar(fraction: fraction)
                                Text(total > 0 ? tr("{read} of {total} chapters · last: {chapter}", ["read": read, "total": total, "chapter": manga.lastChapterName]) : tr("Last: {chapter}", ["chapter": manga.lastChapterName]))
                                    .font(.caption)
                                    .foregroundStyle(.secondary)
                                    .lineLimit(1)
                            }
                        }
                        .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                }
            }
            .padding(16)
            .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 24, style: .continuous))
        }
    }
}

struct StatsSnapshot {
    struct Day: Identifiable {
        let id: Int
        let label: String
        let count: Int
    }

    let chaptersRead: Int
    let pagesRead: Int
    let streak: Int
    let week: [Day]
    let inProgress: [ChapterProgress]
    let readBySeries: [String: Int]

    init(progress: [ChapterProgress]) {
        let calendar = Calendar.current
        let completed = progress.filter { $0.completed }
        let unfinished = progress.filter { !$0.completed && $0.pageCount > 0 }

        chaptersRead = completed.count
        pagesRead = completed.reduce(0) { $0 + $1.pageCount } + unfinished.reduce(0) { $0 + $1.page + 1 }
        inProgress = unfinished

        var perSeries: [String: Int] = [:]
        for item in completed {
            perSeries[item.mangaID, default: 0] += 1
        }
        readBySeries = perSeries

        let activeDays = Set(progress.map { calendar.startOfDay(for: $0.updatedAt) })
        let today = calendar.startOfDay(for: Date())

        // the streak is still alive if yesterday was read but today not yet
        var cursor = today
        if !activeDays.contains(cursor), let yesterday = calendar.date(byAdding: .day, value: -1, to: cursor) {
            cursor = yesterday
        }
        var count = 0
        while activeDays.contains(cursor) {
            count += 1
            guard let previous = calendar.date(byAdding: .day, value: -1, to: cursor) else { break }
            cursor = previous
        }
        streak = count

        let formatter = DateFormatter()
        formatter.dateFormat = "EEEEE"
        var days: [Day] = []
        for offset in stride(from: 6, through: 0, by: -1) {
            guard let day = calendar.date(byAdding: .day, value: -offset, to: today) else { continue }
            let amount = progress.filter { calendar.isDate($0.updatedAt, inSameDayAs: day) }.count
            days.append(Day(id: offset, label: formatter.string(from: day), count: amount))
        }
        week = days
    }
}

struct StatTile: View {
    let value: String
    let label: String
    let icon: String

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Image(systemName: icon)
                .font(.title3)
                .foregroundStyle(.tint)
            Text(value)
                .font(.system(size: 30, weight: .bold, design: .rounded).monospacedDigit())
                .lineLimit(1)
                .minimumScaleFactor(0.6)
            Text(label)
                .font(.footnote)
                .foregroundStyle(.secondary)
                .lineLimit(1)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(16)
        .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 22, style: .continuous))
    }
}

struct ProgressBar: View {
    let fraction: Double
    var height: CGFloat = 5

    var body: some View {
        Capsule()
            .fill(.quaternary)
            .frame(height: height)
            .overlay(alignment: .leading) {
                GeometryReader { proxy in
                    Capsule()
                        .fill(.tint)
                        .frame(width: proxy.size.width * CGFloat(min(max(fraction, 0), 1)))
                }
            }
            .clipShape(Capsule())
    }
}

struct ProgressRing: View {
    let fraction: Double
    var lineWidth: CGFloat = 7

    var body: some View {
        let clamped = min(max(fraction, 0), 1)
        ZStack {
            Circle()
                .stroke(.quaternary, lineWidth: lineWidth)
            Circle()
                .trim(from: 0, to: clamped)
                .stroke(.tint, style: StrokeStyle(lineWidth: lineWidth, lineCap: .round))
                .rotationEffect(.degrees(-90))
            Text("\(Int((clamped * 100).rounded()))%")
                .font(.footnote.weight(.bold).monospacedDigit())
        }
    }
}
