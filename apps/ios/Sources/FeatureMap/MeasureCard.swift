import SwiftUI
import CoreModel
import CoreDesignSystem

/// The measuring card over the map — running distance, undo/clear, done.
struct MeasureCard: View {
    @Environment(\.turbo) private var t
    @Bindable var viewModel: MeasureViewModel
    let onClose: () -> Void

    var body: some View {
        HStack(spacing: 12) {
            Image(systemName: "ruler").foregroundStyle(t.blue)
            VStack(alignment: .leading, spacing: 1) {
                Text(distanceText).font(.turboHeadline).foregroundStyle(t.label)
                Text(viewModel.points.count < 2 ? "Tap the map to measure" : "\(viewModel.points.count) points")
                    .font(.turboCaption).foregroundStyle(t.label2)
            }
            Spacer()
            Button { viewModel.removeLast() } label: { Image(systemName: "arrow.uturn.backward") }
                .disabled(viewModel.points.isEmpty)
            Button { onClose() } label: { Image(systemName: "xmark.circle.fill").foregroundStyle(t.label3) }
                .accessibilityIdentifier("measure.close")
        }
        .padding(14)
        .frame(height: 60)
        .liquidGlass(RoundedRectangle(cornerRadius: 20, style: .continuous), elevated: true)
    }

    private var distanceText: String {
        let m = viewModel.distanceMeters
        return m >= 1000 ? String(format: "%.2f km", m / 1000) : "\(Int(m)) m"
    }
}
