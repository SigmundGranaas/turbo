import SwiftUI
import CoreModel
import CoreDesignSystem

/// The live route-following card over the map — ETA, remaining distance, a
/// progress bar, off-route / arrived state, and Stop. Mirrors Android's follow
/// LiveSheet. Reads the shared ``FollowController`` so it updates with each fix.
struct FollowCard: View {
    @Environment(\.turbo) private var t
    @Bindable var controller: FollowController
    let onStop: () -> Void

    var body: some View {
        VStack(spacing: 10) {
            HStack(spacing: 8) {
                Circle().fill(dotColor).frame(width: 10, height: 10)
                Text(title).font(.turboHeadline).foregroundStyle(t.label).lineLimit(1)
                Spacer()
                Text(etaText)
                    .font(.system(size: 22, weight: .bold, design: .rounded)).monospacedDigit()
                    .foregroundStyle(t.label)
            }

            ProgressView(value: controller.fraction)
                .tint(controller.isOffRoute ? t.orange : t.blue)

            HStack(spacing: 10) {
                Text(distanceText).font(.turboSubhead).foregroundStyle(t.label2)
                if controller.isOffRoute {
                    Label("Off route — rerouting", systemImage: "exclamationmark.triangle.fill")
                        .font(.turboCaption).foregroundStyle(t.orange)
                }
                Spacer()
                Button { onStop() } label: {
                    Text("Stop").font(.turboHeadline).foregroundStyle(.white)
                        .padding(.horizontal, 18).frame(height: 36)
                        .background(t.red, in: Capsule())
                }
                .accessibilityIdentifier("follow.stop")
            }
        }
        .padding(14)
        .liquidGlass(RoundedRectangle(cornerRadius: 20, style: .continuous), elevated: true)
    }

    private var dotColor: Color {
        if controller.arrived { return t.blue }
        return controller.isOffRoute ? t.orange : t.green
    }

    private var title: String {
        if controller.arrived { return "Arrived" }
        return controller.name ?? "Following route"
    }

    private var etaText: String {
        if controller.arrived { return "Done" }
        guard let s = controller.etaSeconds else { return "—" }
        if s >= 3600 { return String(format: "%d:%02d h", s / 3600, (s % 3600) / 60) }
        return "\(max(1, s / 60)) min"
    }

    private var distanceText: String {
        let m = controller.distanceRemainingM
        let value = m >= 1000 ? String(format: "%.1f km", m / 1000) : "\(Int(m)) m"
        return controller.arrived ? "You've reached the end" : "\(value) left"
    }
}
