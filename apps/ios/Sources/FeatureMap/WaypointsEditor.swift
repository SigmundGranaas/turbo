import SwiftUI
import CoreModel
import CoreDesignSystem

/// Manage a route's stops — reorder (drag) and remove (swipe) — mirroring
/// Android's WaypointsSheet. Editing here re-solves the route live.
struct WaypointsEditor: View {
    @Environment(\.turbo) private var t
    @Environment(\.dismiss) private var dismiss
    @Bindable var viewModel: RouteViewModel

    var body: some View {
        NavigationStack {
            List {
                Section {
                    ForEach(Array(viewModel.waypoints.enumerated()), id: \.offset) { index, point in
                        HStack(spacing: 12) {
                            badge(index)
                            Text(Geo.formatCoords(point)).font(.turboBody).foregroundStyle(t.label)
                            Spacer()
                        }
                    }
                    .onMove { source, destination in
                        guard let from = source.first else { return }
                        viewModel.moveWaypoint(from: from, to: destination > from ? destination - 1 : destination)
                    }
                    .onDelete { offsets in
                        if let index = offsets.first { viewModel.removeWaypoint(at: index) }
                    }
                } footer: {
                    Text("Drag to reorder, swipe to remove. The route re-solves as you edit.")
                }
            }
            #if os(iOS)
            .environment(\.editMode, .constant(.active))
            #endif
            .navigationTitle("Stops")
            .toolbarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .confirmationAction) { Button("Done") { dismiss() } } }
        }
    }

    /// Green start, red end, blue numbered vias.
    private func badge(_ index: Int) -> some View {
        let isStart = index == 0
        let isEnd = index == viewModel.waypoints.count - 1
        let color = isStart ? t.green : (isEnd ? t.red : t.blue)
        let label = isStart ? "S" : (isEnd ? "E" : "\(index + 1)")
        return Text(label)
            .font(.turboFootnote.weight(.bold)).foregroundStyle(.white)
            .frame(width: 28, height: 28)
            .background(color, in: Circle())
    }
}
