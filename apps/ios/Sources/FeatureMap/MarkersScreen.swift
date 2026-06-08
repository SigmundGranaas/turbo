import SwiftUI
import CoreModel
import CoreDesignSystem

/// "My Markers" — every spot the hiker has saved, with export. The browsable home
/// for markers, mirroring the marker-list role in Android's `feature.markers`.
public struct MarkersScreen: View {
    @Environment(\.turbo) private var t
    @State private var viewModel: MarkersViewModel

    public init(viewModel: MarkersViewModel) {
        _viewModel = State(initialValue: viewModel)
    }

    public var body: some View {
        List {
            if viewModel.markers.isEmpty {
                ContentUnavailableView(
                    "No Markers Yet",
                    systemImage: "mappin.slash",
                    description: Text("Long-press the map or tap + to save a spot.")
                )
            } else {
                ForEach(viewModel.markers) { marker in
                    NavigationLink(value: marker) { MarkerRow(marker: marker) }
                        .swipeActions {
                            Button(role: .destructive) { viewModel.delete(id: marker.id) } label: {
                                Label("Delete", systemImage: "trash")
                            }
                        }
                        .contextMenu {
                            if let url = try? MarkerExport.writeTemporaryFile(marker) {
                                ShareLink(item: url) { Label("Export GPX", systemImage: "square.and.arrow.up") }
                            }
                        }
                }
            }
        }
        .navigationTitle("My Markers")
        .navigationDestination(for: Marker.self) { marker in
            MarkerDetailScreen(marker: marker, onDelete: { viewModel.delete(id: marker.id) })
        }
        .task { viewModel.start() }
    }
}

private struct MarkerRow: View {
    @Environment(\.turbo) private var t
    let marker: Marker

    var body: some View {
        HStack(spacing: 13) {
            Glyph(symbol: marker.kind.symbolName, color: marker.displayColor(t), size: 38, cornerRadius: 10)
            VStack(alignment: .leading, spacing: 1) {
                Text(marker.name).font(.turboHeadline).foregroundStyle(t.label)
                Text("\(marker.kind.label) · \(Geo.formatCoords(marker.position))")
                    .font(.turboFootnote).foregroundStyle(t.label2)
            }
            Spacer(minLength: 8)
        }
        .padding(.vertical, 2)
    }
}
