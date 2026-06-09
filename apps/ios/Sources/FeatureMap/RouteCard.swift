import SwiftUI
import CoreModel
import CoreDesignSystem

/// The route-building card over the map — mode (snap-to-trail vs straight),
/// preset, live stats, and save/undo/clear. Mirrors the design's route tool.
struct RouteCard: View {
    @Environment(\.turbo) private var t
    @Bindable var viewModel: RouteViewModel
    let onClose: () -> Void
    @State private var showSave = false
    @State private var showStops = false
    @State private var name = ""

    var body: some View {
        VStack(spacing: 12) {
            HStack {
                Text("Plan a Route").font(.turboHeadline).foregroundStyle(t.label)
                Spacer()
                Button { onClose() } label: {
                    Image(systemName: "xmark.circle.fill").foregroundStyle(t.label3)
                }
                .accessibilityIdentifier("route.close")
            }

            Picker("Mode", selection: Binding(get: { viewModel.mode }, set: { viewModel.setMode($0) })) {
                ForEach(RouteViewModel.Mode.allCases, id: \.self) { Text($0.label).tag($0) }
            }
            .pickerStyle(.segmented)

            if let plan = viewModel.plan {
                HStack(spacing: 18) {
                    stat(String(format: "%.1f km", plan.distanceM / 1000), "Distance")
                    stat(durationText(plan.durationS), "Time")
                    if plan.ascentM > 0 { stat("\(Int(plan.ascentM)) m", "Ascent") }
                    Spacer()
                }
            } else if viewModel.isSolving {
                HStack(spacing: 8) { ProgressView(); Text("Solving…").font(.turboFootnote).foregroundStyle(t.label2); Spacer() }
            } else {
                Text("Tap the map to add points.").font(.turboFootnote).foregroundStyle(t.label2)
                    .frame(maxWidth: .infinity, alignment: .leading)
            }

            HStack(spacing: 16) {
                Button { showStops = true } label: { Label("\(viewModel.waypoints.count)", systemImage: "list.bullet") }
                    .disabled(viewModel.waypoints.isEmpty)
                    .accessibilityIdentifier("route.stops")
                    .accessibilityLabel("Edit stops")
                Button { viewModel.undo() } label: { Image(systemName: "arrow.uturn.backward") }
                    .disabled(!viewModel.canUndo)
                    .accessibilityLabel("Undo")
                Button { viewModel.clear() } label: { Image(systemName: "trash") }
                    .disabled(viewModel.waypoints.isEmpty)
                    .accessibilityLabel("Clear route")
                Spacer()
                Button { name = ""; showSave = true } label: {
                    Text("Save").font(.turboHeadline).foregroundStyle(.white)
                        .padding(.horizontal, 16).frame(height: 38)
                        .background(t.blue, in: Capsule())
                }
                .disabled(viewModel.plan == nil)
                .accessibilityIdentifier("route.save")
            }
            .font(.turboSubhead)
        }
        .padding(14)
        .liquidGlass(RoundedRectangle(cornerRadius: 20, style: .continuous), elevated: true)
        .alert("Save Route", isPresented: $showSave) {
            TextField("Name", text: $name)
            Button("Save") { viewModel.saveAsPath(name: name); onClose() }
            Button("Cancel", role: .cancel) {}
        }
        .sheet(isPresented: $showStops) {
            WaypointsEditor(viewModel: viewModel)
                .presentationDetents([.medium, .large])
        }
    }

    private func stat(_ value: String, _ label: String) -> some View {
        VStack(alignment: .leading, spacing: 1) {
            Text(value).font(.turboHeadline).foregroundStyle(t.label)
            Text(label).font(.turboCaption).foregroundStyle(t.label2)
        }
    }

    private func durationText(_ seconds: Double) -> String {
        let m = Int(seconds / 60)
        return m >= 60 ? "\(m / 60)h \(m % 60)m" : "\(m) min"
    }
}
