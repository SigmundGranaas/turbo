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
    @State private var autoUpdate = true
    private let onBack: (() -> Void)?

    public init(viewModel: OfflineViewModel, onBack: (() -> Void)? = nil) {
        _viewModel = State(initialValue: viewModel)
        self.onBack = onBack
    }

    public var body: some View {
        List {
            Section {
                ForEach(viewModel.regions) { region in
                    OfflineRegionRow(region: region) { viewModel.delete(id: region.id) }
                }
            } footer: {
                Text(storageFooter)
            }

            Section {
                Button(action: downloadSample) {
                    Label("Download New Region", systemImage: "plus")
                }
            }

            Section {
                Toggle("Auto-update over Wi-Fi", isOn: $autoUpdate)
            }
        }
        .navigationTitle("Offline Maps")
        .toolbarTitleDisplayMode(.inlineLarge)
        .toolbar {
            ToolbarItem(placement: .primaryAction) {
                Button(action: downloadSample) {
                    Image(systemName: "plus")
                }
                .accessibilityLabel("Download region")
            }
        }
        .task { viewModel.start() }
    }

    private var storageFooter: String {
        let used = viewModel.regions.reduce(Int64(0)) { $0 + $1.sizeBytes }
        return "Offline regions stay current for trips out of signal. \(Geo.formatBytes(used)) of 64 GB used."
    }

    /// Demo download — the real flow downloads the current map camera box.
    private func downloadSample() {
        let lyngen = GeoBounds(south: 69.45, west: 19.80, north: 69.75, east: 20.40)
        viewModel.download(name: "Lyngen Alps", base: .norgeskart, bounds: lyngen, fromZoom: 11)
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
