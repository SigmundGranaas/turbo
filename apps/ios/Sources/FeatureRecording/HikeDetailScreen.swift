import SwiftUI
import CoreModel
import CoreDesignSystem

/// A completed hike — route stats, elevation profile, and GPX export. Mirrors the
/// "Hike detail" design (the Apple Health rings / heart-rate are a later add).
public struct HikeDetailScreen: View {
    @Environment(\.turbo) private var t
    private let path: SavedPath
    private var stats: HikeStats { HikeStats(path.path) }

    public init(path: SavedPath) { self.path = path }

    public var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 20) {
                if !stats.elevationProfile.isEmpty {
                    VStack(alignment: .leading, spacing: 8) {
                        Text("Elevation").font(.turboFootnote).foregroundStyle(t.label2).textCase(.uppercase)
                        Spark(data: stats.elevationProfile, color: tint)
                            .frame(height: 120)
                            .padding(14)
                            .background(t.groupedCard, in: RoundedRectangle(cornerRadius: Radius.card, style: .continuous))
                    }
                }

                LazyVGrid(columns: [GridItem(.flexible()), GridItem(.flexible())], spacing: 12) {
                    statTile("Distance", stats.formattedDistance, "ruler")
                    statTile("Duration", stats.formattedDuration ?? "—", "clock")
                    statTile("Ascent", stats.ascentMeters.map { "\(Int($0)) m" } ?? "—", "arrow.up.right")
                    statTile("Avg Pace", stats.formattedPace ?? "—", "speedometer")
                }

                if let url = try? TrackExport.writeTemporaryFile(path, as: .gpx) {
                    ShareLink(item: url) {
                        Label("Export GPX", systemImage: "square.and.arrow.up")
                            .font(.turboHeadline)
                            .frame(maxWidth: .infinity, minHeight: 50)
                            .background(t.fill3, in: RoundedRectangle(cornerRadius: 14, style: .continuous))
                            .foregroundStyle(t.blue)
                    }
                }
            }
            .padding(16)
        }
        .background(t.grouped)
        .navigationTitle(path.name)
    }

    private var tint: Color { path.activityKind?.tint(t) ?? t.blue }

    private func statTile(_ label: String, _ value: String, _ symbol: String) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            Image(systemName: symbol).foregroundStyle(tint)
            Text(value).font(.turboTitle2).foregroundStyle(t.label)
            Text(label).font(.turboFootnote).foregroundStyle(t.label2)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(14)
        .background(t.groupedCard, in: RoundedRectangle(cornerRadius: Radius.card, style: .continuous))
    }
}
