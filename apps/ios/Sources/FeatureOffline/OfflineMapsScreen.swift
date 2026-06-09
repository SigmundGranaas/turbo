import SwiftUI
import CoreModel
import CoreMap
import CoreDesignSystem

/// Offline maps — downloaded regions with live download progress, plus the
/// entry points to download a new region and toggle Wi-Fi auto-update.
///
/// Native grouped table view, mirroring `OfflineMaps` in the design bundle
/// (`ios/iosMore2.jsx`). Mirrors `feature.offline.OfflineMapsScreen` (Android).
public struct OfflineMapsScreen: View {
    @Environment(\.turbo) private var t
    @State private var viewModel: OfflineViewModel
    private let onBack: (() -> Void)?
    /// The map's current visible rectangle — what "Download" grabs. Nil if the
    /// map hasn't reported a region yet (download is then unavailable).
    private let currentBounds: GeoBounds?
    private let currentBase: BaseLayer
    /// Resolve a human name for a downloaded area (reverse-geocode), else nil.
    private let resolveName: ((GeoBounds) async -> String?)?

    public init(viewModel: OfflineViewModel, currentBounds: GeoBounds? = nil,
                currentBase: BaseLayer = .norgeskart,
                resolveName: ((GeoBounds) async -> String?)? = nil,
                onBack: (() -> Void)? = nil) {
        _viewModel = State(initialValue: viewModel)
        self.currentBounds = currentBounds
        self.currentBase = currentBase
        self.resolveName = resolveName
        self.onBack = onBack
    }

    public var body: some View {
        List {
            Section {
                if viewModel.regions.isEmpty {
                    ContentUnavailableView(
                        "No Offline Maps",
                        systemImage: "square.and.arrow.down",
                        description: Text("Download a region to keep its map tiles available off the grid.")
                    )
                }
                ForEach(viewModel.regions) { region in
                    OfflineRegionRow(region: region) { viewModel.delete(id: region.id) }
                }
            } footer: {
                Text(storageFooter)
            }

            Section {
                Button(action: downloadCurrentArea) {
                    Label("Download This Area", systemImage: "square.and.arrow.down")
                }
                .disabled(currentBounds == nil)
            } footer: {
                if currentBounds == nil {
                    Text("Open the map and pan to the area you want before downloading.")
                }
            }
        }
        .navigationTitle("Offline Maps")
        .toolbarTitleDisplayMode(.inlineLarge)
        .toolbar {
            ToolbarItem(placement: .primaryAction) {
                Button(action: downloadCurrentArea) {
                    Image(systemName: "square.and.arrow.down")
                }
                .accessibilityLabel("Download this area")
                .disabled(currentBounds == nil)
            }
        }
        .task { viewModel.start() }
    }

    private var storageFooter: String {
        let used = viewModel.regions.reduce(Int64(0)) { $0 + $1.sizeBytes }
        let base = "Offline regions stay current for trips out of signal."
        guard let free = Self.freeDiskBytes() else { return "\(base) \(Geo.formatBytes(used)) downloaded." }
        return "\(base) \(Geo.formatBytes(used)) downloaded · \(Geo.formatBytes(free)) free."
    }

    /// Real free space on the device's volume.
    private static func freeDiskBytes() -> Int64? {
        let url = URL(fileURLWithPath: NSHomeDirectory())
        let values = try? url.resourceValues(forKeys: [.volumeAvailableCapacityForImportantUsageKey])
        return values?.volumeAvailableCapacityForImportantUsage
    }

    /// Download the area currently shown on the map, at a zoom matched to the
    /// viewport (plus a few more detailed levels) and named via reverse-geocode.
    private func downloadCurrentArea() {
        guard let bounds = currentBounds else { return }
        let lonSpan = max(bounds.east - bounds.west, 0.0001)
        let fromZoom = min(15, max(8, log2(360 / lonSpan)))
        Task {
            let name = await resolveName?(bounds) ?? Self.coordinateName(bounds)
            viewModel.download(name: name, base: currentBase, bounds: bounds, fromZoom: fromZoom)
        }
    }

    /// Fallback name from the area's centre, e.g. "Area 69.6°N 19.9°E".
    private static func coordinateName(_ b: GeoBounds) -> String {
        let lat = (b.south + b.north) / 2, lng = (b.west + b.east) / 2
        return String(format: "Area %.1f°N %.1f°E", lat, lng)
    }
}

/// One region row — a live download (icon + progress bar) or a completed region.
private struct OfflineRegionRow: View {
    @Environment(\.turbo) private var t
    let region: OfflineRegionInfo
    let onDelete: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(spacing: 13) {
                Glyph(
                    symbol: region.complete ? "checkmark" : "arrow.down",
                    color: region.complete ? t.green : t.blue,
                    size: 38, cornerRadius: 10
                )
                VStack(alignment: .leading, spacing: 1) {
                    Text(region.name)
                        .font(.turboHeadline)
                        .foregroundStyle(t.label)
                    Text(subtitle)
                        .font(.turboFootnote)
                        .foregroundStyle(t.label2)
                }
                Spacer(minLength: 8)
                if region.complete {
                    Menu {
                        Button(role: .destructive, action: onDelete) {
                            Label("Delete", systemImage: "trash")
                        }
                    } label: {
                        Image(systemName: "ellipsis")
                            .foregroundStyle(t.label3)
                    }
                } else {
                    Button(action: onDelete) {
                        Image(systemName: "xmark.circle.fill")
                            .foregroundStyle(t.label3)
                    }
                    .buttonStyle(.plain)
                    .accessibilityLabel("Cancel download")
                }
            }
            if !region.complete {
                ProgressView(value: region.progress)
                    .tint(t.blue)
            }
        }
        .padding(.vertical, 2)
        .swipeActions {
            Button(role: .destructive, action: onDelete) {
                Label("Delete", systemImage: "trash")
            }
        }
    }

    private var subtitle: String {
        if region.complete {
            "\(Geo.formatBytes(region.sizeBytes)) · \(layerNames)"
        } else {
            "Downloading · \(Int(region.progress * 100))% · \(Geo.formatBytes(region.sizeBytes))"
        }
    }

    private var layerNames: String {
        region.layers.map(\.shortTitle).joined(separator: " + ")
    }
}

private extension BaseLayer {
    /// Short label used in the offline subtitle ("Topo + Satellite").
    var shortTitle: String {
        switch self {
        case .norgeskart: "Topo"
        case .osm: "OSM"
        case .satellite: "Satellite"
        }
    }
}

#Preview {
    TurboTheme {
        NavigationStack {
            OfflineMapsScreen(
                viewModel: OfflineViewModel(manager: InMemoryOfflineTileManager())
            )
        }
    }
}
