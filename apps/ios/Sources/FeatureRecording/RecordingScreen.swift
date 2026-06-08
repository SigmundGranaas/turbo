import SwiftUI
import CoreDesignSystem

/// Live track recording — elapsed time + distance update as the hiker moves;
/// Stop offers to name and save the track. A thin observer of the shared
/// ``RecordingController``: the session lives in the container, so dismissing this
/// sheet (Minimize) keeps recording while the map's recording pill stays visible.
public struct RecordingScreen: View {
    @Environment(\.turbo) private var t
    @Environment(\.dismiss) private var dismiss
    private let controller: RecordingController
    @State private var showSave = false
    @State private var name = ""

    public init(controller: RecordingController) {
        self.controller = controller
    }

    public var body: some View {
        VStack(spacing: 28) {
            HStack {
                Button { dismiss() } label: {
                    Label("Minimize", systemImage: "chevron.down")
                        .labelStyle(.iconOnly)
                        .font(.system(size: 17, weight: .semibold))
                        .foregroundStyle(t.label2)
                        .frame(width: 44, height: 44)
                }
                .accessibilityLabel("Keep recording and return to the map")
                Spacer()
            }
            .padding(.horizontal, 12)
            .padding(.top, 12)

            HStack(spacing: 8) {
                Circle().fill(controller.isRecording ? t.red : t.orange).frame(width: 12, height: 12)
                Text(controller.isRecording ? "Recording" : "Paused").font(.turboHeadline).foregroundStyle(t.label)
            }

            VStack(spacing: 6) {
                Text(elapsed)
                    .font(.system(size: 64, weight: .bold, design: .rounded))
                    .monospacedDigit()
                    .foregroundStyle(t.label)
                    .accessibilityIdentifier("recording.elapsed")
                Text(String(format: "%.2f km", controller.distanceMeters / 1000))
                    .font(.turboTitle2)
                    .foregroundStyle(t.label2)
                    .accessibilityIdentifier("recording.distance")
            }

            Spacer()

            Button {
                controller.stop()
                name = ""
                showSave = true
            } label: {
                Text("Stop")
                    .font(.turboHeadline)
                    .foregroundStyle(.white)
                    .frame(maxWidth: .infinity, minHeight: 54)
                    .background(t.red, in: RoundedRectangle(cornerRadius: 16, style: .continuous))
            }
            .accessibilityIdentifier("recording.stop")
            .padding(.horizontal, 24)
            .padding(.bottom, 32)
        }
        .background(t.background)
        .task { if !controller.isSessionActive { controller.start() } }
        .alert("Save Track", isPresented: $showSave) {
            TextField("Name", text: $name)
            Button("Save") { controller.save(name: name); dismiss() }
            Button("Discard", role: .destructive) { controller.discard(); dismiss() }
            Button("Keep Recording", role: .cancel) { controller.resume() }
        } message: {
            Text("Name this track to save it to your paths.")
        }
    }

    private var elapsed: String {
        let s = controller.elapsedSeconds
        return String(format: "%02d:%02d", s / 60, s % 60)
    }
}
