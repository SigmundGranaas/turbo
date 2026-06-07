import SwiftUI
import CoreModel
import CoreDesignSystem

/// All recorded tracks with elevation sparklines, plus the entry points to record
/// or draw a new one. Mirrors `PathsList` (design) / `feature.recording.PathsListScreen`.
public struct PathsScreen: View {
    @Environment(\.turbo) private var t
    @State private var viewModel: PathsViewModel

    public init(viewModel: PathsViewModel) {
        _viewModel = State(initialValue: viewModel)
    }

    public var body: some View {
        List {
            Section {
                ForEach(viewModel.paths) { path in
                    PathRow(path: path)
                        .swipeActions {
                            Button(role: .destructive) { viewModel.delete(id: path.id) } label: {
                                Label("Delete", systemImage: "trash")
                            }
                        }
                        .contextMenu { exportMenu(for: path) }
                }
            }

            Section {
                Label("Record New Path", systemImage: "record.circle")
                    .foregroundStyle(t.red)
                Label("Draw a Route", systemImage: "pencil.line")
                    .foregroundStyle(t.blue)
            }
        }
        .navigationTitle("Paths")
        .task { viewModel.start() }
    }

    /// Export options — a `ShareLink` per format. The temp file is written lazily
    /// when the context menu is opened.
    @ViewBuilder
    private func exportMenu(for path: SavedPath) -> some View {
        ForEach(ExportFormat.allCases, id: \.self) { format in
            if let url = try? TrackExport.writeTemporaryFile(path, as: format) {
                ShareLink(item: url) {
                    Label("Export \(format.label)", systemImage: "square.and.arrow.up")
                }
            }
        }
    }
}

private struct PathRow: View {
    @Environment(\.turbo) private var t
    let path: SavedPath

    var body: some View {
        HStack(spacing: 13) {
            Glyph(symbol: "point.topleft.down.curvedto.point.bottomright.up", color: tint, size: 38, cornerRadius: 10)
            VStack(alignment: .leading, spacing: 1) {
                Text(path.name).font(.turboHeadline).foregroundStyle(t.label)
                Text(meta).font(.turboFootnote).foregroundStyle(t.label2)
            }
            Spacer(minLength: 8)
            if let elevations = path.path.elevations, elevations.count > 1 {
                Spark(data: elevations, color: tint)
                    .frame(width: 62, height: 30)
            }
        }
        .padding(.vertical, 2)
    }

    private var tint: Color { path.activityKind?.tint(t) ?? t.blue }

    private var meta: String {
        let km = path.path.distanceM / 1000
        let distance = String(format: "%.1f km", km)
        if let kind = path.activityKind { return "\(distance) · \(kind.label)" }
        return distance
    }
}
