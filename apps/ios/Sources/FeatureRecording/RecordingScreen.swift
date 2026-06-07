import SwiftUI
import CoreDesignSystem

/// Live track recording — elapsed time + distance update as the hiker moves;
/// Stop offers to name and save the track. Mirrors the Recording screen in the
/// design (the Live Activity / Dynamic Island surface is a later follow-on).
public struct RecordingScreen: View {
    @Environment(\.turbo) private var t
    @Environment(\.dismiss) private var dismiss
    @State private var viewModel: RecordingViewModel
    @State private var showSave = false
    @State private var name = ""

    public init(viewModel: RecordingViewModel) {
        _viewModel = State(initialValue: viewModel)
    }

    public var body: some View {
        VStack(spacing: 28) {
            HStack(spacing: 8) {
                Circle().fill(t.red).frame(width: 12, height: 12)
                Text("Recording").font(.turboHeadline).foregroundStyle(t.label)
            }
            .padding(.top, 48)

            VStack(spacing: 6) {
                Text(elapsed)
                    .font(.system(size: 64, weight: .bold, design: .rounded))
                    .monospacedDigit()
                    .foregroundStyle(t.label)
                    .accessibilityIdentifier("recording.elapsed")
                Text(String(format: "%.2f km", viewModel.distanceMeters / 1000))
                    .font(.turboTitle2)
                    .foregroundStyle(t.label2)
                    .accessibilityIdentifier("recording.distance")
            }

            Spacer()

            Button {
                viewModel.stop()
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
        .task { viewModel.start() }
        .alert("Save Track", isPresented: $showSave) {
            TextField("Name", text: $name)
            Button("Save") { viewModel.save(name: name); dismiss() }
            Button("Discard", role: .destructive) { viewModel.discard(); dismiss() }
            Button("Keep Recording", role: .cancel) { viewModel.start() }
        } message: {
            Text("Name this track to save it to your paths.")
        }
    }

    private var elapsed: String {
        let s = viewModel.elapsedSeconds
        return String(format: "%02d:%02d", s / 60, s % 60)
    }
}
