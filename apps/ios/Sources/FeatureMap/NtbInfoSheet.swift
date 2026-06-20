import SwiftUI
import CoreModel
import CoreDesignSystem

/// Bottom sheet for a tapped Nasjonal Turbase (ut.no / DNT) POI — cabin, trip or
/// place. Shows the title, summary and (for a trip) distance/grade chips, plus a
/// deep link back to ut.no. The trip's route is revealed on the map behind it.
/// Mirrors `feature.map.NtbInfoSheet` (Android).
struct NtbInfoSheet: View {
    @Environment(\.turbo) private var t
    @Environment(\.openURL) private var openURL
    @Environment(\.dismiss) private var dismiss

    let poi: NtbPoi
    let route: NtbRoute?

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 14) {
                    HStack(spacing: 12) {
                        Image(systemName: poi.symbolName)
                            .font(.system(size: 26))
                            .foregroundStyle(t.red)
                        Text(poi.title)
                            .font(.turboTitle3)
                            .foregroundStyle(t.label)
                    }

                    if let summary = poi.summary, !summary.isEmpty {
                        Text(summary)
                            .font(.turboBody)
                            .foregroundStyle(t.label2)
                    }

                    if route?.distanceMeters != nil || route?.grade != nil {
                        HStack(spacing: 8) {
                            if let d = route?.distanceMeters {
                                InfoChip(symbol: "ruler", label: Self.formatDistance(d), t: t)
                            }
                            if let grade = route?.grade, !grade.isEmpty {
                                InfoChip(symbol: "figure.hiking", label: grade, t: t)
                            }
                        }
                    }

                    if let link = (route?.utUrl ?? poi.utUrl).flatMap(URL.init(string:)) {
                        Button {
                            openURL(link)
                        } label: {
                            HStack(spacing: 8) {
                                Image(systemName: "arrow.up.right.square")
                                Text("Open in UT.no").font(.turboHeadline)
                            }
                            .frame(maxWidth: .infinity)
                            .frame(height: 48)
                            .background(t.groupedCard)
                            .clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
                            .foregroundStyle(t.blue)
                        }
                        .buttonStyle(.plain)
                        .padding(.top, 6)
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(20)
            }
            .background(t.grouped)
            .toolbar {
                ToolbarItem(placement: .confirmationAction) {
                    Button("Done") { dismiss() }
                }
            }
        }
    }

    /// Compact distance label (m below 1 km, else 1-decimal km).
    static func formatDistance(_ meters: Double) -> String {
        meters >= 1000
            ? String(format: "%.1f km", meters / 1000)
            : "\(Int(meters.rounded())) m"
    }
}

private struct InfoChip: View {
    let symbol: String
    let label: String
    let t: TurboColors

    var body: some View {
        HStack(spacing: 6) {
            Image(systemName: symbol).foregroundStyle(t.blue)
            Text(label).font(.turboSubhead).foregroundStyle(t.label)
        }
        .padding(.horizontal, 12)
        .frame(height: 34)
        .background(t.groupedCard)
        .clipShape(Capsule())
    }
}

extension NtbPoi {
    /// SF Symbol for this POI's type — shared by the map pin and the info sheet.
    var symbolName: String {
        switch type {
        case .cabin: "house.lodge"
        case .trip: "figure.hiking"
        case .place: "mappin"
        }
    }
}
