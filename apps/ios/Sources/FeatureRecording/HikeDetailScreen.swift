import SwiftUI
import CoreModel
import CoreDesignSystem

/// A completed hike — route stats, elevation profile, and GPX export. Mirrors the
/// "Hike detail" design (the Apple Health rings / heart-rate are a later add).
public struct HikeDetailScreen: View {
    @Environment(\.turbo) private var t
    private let path: SavedPath
    private let shareResource: ((String) async -> URL?)?
    private let onFollow: (() -> Void)?
    private var stats: HikeStats { HikeStats(path.path) }

    public init(path: SavedPath, shareResource: ((String) async -> URL?)? = nil, onFollow: (() -> Void)? = nil) {
        self.path = path
        self.shareResource = shareResource
        self.onFollow = onFollow
    }

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

                // Checkpoint splits captured while following the planned route (D1 / US-3).
                if !path.phaseSplits.isEmpty {
                    VStack(alignment: .leading, spacing: 8) {
                        Text("Checkpoints").font(.turboFootnote).foregroundStyle(t.label2).textCase(.uppercase)
                        VStack(spacing: 0) {
                            ForEach(Array(path.phaseSplits.enumerated()), id: \.offset) { index, split in
                                if index > 0 { Divider().overlay(t.separator) }
                                splitRow(split)
                            }
                        }
                        .padding(.vertical, 4)
                        .background(t.groupedCard, in: RoundedRectangle(cornerRadius: Radius.card, style: .continuous))
                    }
                }

                if let onFollow, path.path.points.count >= 2 {
                    Button(action: onFollow) {
                        Label("Follow This Track", systemImage: "location.north.fill")
                            .font(.turboHeadline)
                            .frame(maxWidth: .infinity, minHeight: 50)
                            .background(t.green, in: RoundedRectangle(cornerRadius: 14, style: .continuous))
                            .foregroundStyle(.white)
                    }
                    .accessibilityIdentifier("hike.follow")
                }

                if let url = try? TrackExport.writeTemporaryFile(path, as: .gpx) {
                    ShareLink(item: url) {
                        actionLabel("Export GPX", "square.and.arrow.up")
                    }
                }

                if let shareResource {
                    ShareLinkButton(create: { await shareResource(path.id) }) {
                        actionLabel("Share Link", "person.2")
                    }
                    .accessibilityIdentifier("hike.share")
                }
            }
            .padding(16)
        }
        .background(t.grouped)
        .navigationTitle(path.name)
    }

    private var tint: Color { path.activityKind?.tint(t) ?? t.blue }

    private func actionLabel(_ title: String, _ symbol: String) -> some View {
        Label(title, systemImage: symbol)
            .font(.turboHeadline)
            .frame(maxWidth: .infinity, minHeight: 50)
            .background(t.fill3, in: RoundedRectangle(cornerRadius: 14, style: .continuous))
            .foregroundStyle(t.blue)
    }

    /// One checkpoint split: name on the left, time + distance since the previous one on the right.
    private func splitRow(_ split: PhaseSplit) -> some View {
        HStack {
            Image(systemName: "flag.checkered").font(.footnote).foregroundStyle(tint)
            Text(split.name).font(.turboBody).foregroundStyle(t.label)
            Spacer()
            Text("\(formatClock(split.splitSeconds)) · \(formatDistance(split.splitDistanceM))")
                .font(.turboFootnote).foregroundStyle(t.label2)
        }
        .padding(.horizontal, 14)
        .padding(.vertical, 10)
    }

    /// Seconds → "m:ss" (or "h:mm:ss" past an hour).
    private func formatClock(_ seconds: Int) -> String {
        let s = max(0, seconds)
        let h = s / 3600, m = (s % 3600) / 60, sec = s % 60
        return h > 0 ? String(format: "%d:%02d:%02d", h, m, sec) : String(format: "%d:%02d", m, sec)
    }

    /// Metres → "1.8 km" past a kilometre, else "740 m".
    private func formatDistance(_ meters: Double) -> String {
        meters >= 1000 ? String(format: "%.1f km", meters / 1000) : "\(Int(meters.rounded())) m"
    }

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
