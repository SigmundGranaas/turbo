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
    @State private var showResumeBuffer = false
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
                .accessibilityIdentifier("recording.minimize")
                Spacer()
            }
            .padding(.horizontal, 12)
            .padding(.top, 12)

            HStack(spacing: 8) {
                let paused = controller.isPausedBuffering || !controller.isRecording
                Circle().fill(paused ? t.orange : t.red).frame(width: 12, height: 12)
                Text(paused ? "Paused" : "Recording").font(.turboHeadline).foregroundStyle(t.label)
            }

            Text(elapsed)
                .font(.system(size: 60, weight: .bold, design: .rounded))
                .monospacedDigit()
                .foregroundStyle(t.label)
                .accessibilityIdentifier("recording.elapsed")

            LazyVGrid(columns: [GridItem(.flexible()), GridItem(.flexible()), GridItem(.flexible())], spacing: 16) {
                statTile(String(format: "%.2f", controller.distanceMeters / 1000), "km", "ruler", id: "recording.distance")
                statTile("\(Int(controller.ascentMeters))", "m ↑", "arrow.up.right")
                statTile("\(Int(controller.descentMeters))", "m ↓", "arrow.down.right")
                statTile(speedText, "km/h", "speedometer")
                statTile(paceText, "/km", "figure.walk")
                statTile(altitudeText, "m alt", "mountain.2")
            }
            .padding(.horizontal, 20)

            Spacer()

            // Proactive nudge (US-4): you walked while paused — catch it with a banner + Resume.
            if controller.isPausedBuffering && controller.hasBufferedMovement {
                HStack(spacing: 10) {
                    Image(systemName: "figure.walk").foregroundStyle(t.label)
                    VStack(alignment: .leading, spacing: 1) {
                        Text("Still moving while paused?").font(.turboSubhead).foregroundStyle(t.label)
                        Text("\(Int(controller.bufferedDistanceM)) m captured · resume to keep it")
                            .font(.turboCaption).foregroundStyle(t.label2)
                    }
                    Spacer()
                    Button("Resume") { showResumeBuffer = true }
                        .font(.turboHeadline).foregroundStyle(t.label)
                }
                .padding(12)
                .background(t.groupedCard, in: RoundedRectangle(cornerRadius: 14, style: .continuous))
                .padding(.horizontal, 24)
                .accessibilityIdentifier("recording.nudge")
            }

            HStack(spacing: 12) {
                // Pause keeps capturing into a buffer; resuming asks Include/Discard if you
                // walked while paused (US-4). A one-tap resume with no buffer just continues.
                Button {
                    if controller.isPausedBuffering {
                        if controller.hasBufferedMovement { showResumeBuffer = true } else { controller.resume(includeBuffered: false) }
                    } else {
                        controller.pause()
                    }
                } label: {
                    Label(controller.isPausedBuffering ? "Resume" : "Pause",
                          systemImage: controller.isPausedBuffering ? "play.fill" : "pause.fill")
                        .font(.turboHeadline)
                        .foregroundStyle(t.label)
                        .frame(maxWidth: .infinity, minHeight: 54)
                        .background(t.groupedCard, in: RoundedRectangle(cornerRadius: 16, style: .continuous))
                }
                .accessibilityIdentifier("recording.pause")

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
            }
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
        .alert("Moved while paused", isPresented: $showResumeBuffer) {
            Button("Include") { controller.resume(includeBuffered: true) }
            Button("Discard", role: .destructive) { controller.resume(includeBuffered: false) }
        } message: {
            Text("You covered \(Int(controller.bufferedDistanceM)) m while paused. Add it to your track?")
        }
    }

    private func statTile(_ value: String, _ unit: String, _ symbol: String, id: String? = nil) -> some View {
        VStack(spacing: 4) {
            Image(systemName: symbol).font(.system(size: 15)).foregroundStyle(t.label3)
            Text(value)
                .font(.system(size: 26, weight: .semibold, design: .rounded)).monospacedDigit()
                .foregroundStyle(t.label)
                .accessibilityIdentifier(id ?? "")
            Text(unit).font(.turboCaption).foregroundStyle(t.label2)
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, 14)
        .background(t.groupedCard, in: RoundedRectangle(cornerRadius: 14, style: .continuous))
    }

    private var elapsed: String {
        let s = controller.elapsedSeconds
        let h = s / 3600
        return h > 0 ? String(format: "%d:%02d:%02d", h, (s % 3600) / 60, s % 60)
                     : String(format: "%02d:%02d", s / 60, s % 60)
    }

    private var speedText: String {
        guard let mps = controller.currentSpeedMps else { return "—" }
        return String(format: "%.1f", mps * 3.6)
    }

    private var paceText: String {
        guard let pace = controller.paceSecondsPerKm else { return "—" }
        return String(format: "%d'%02d", pace / 60, pace % 60)
    }

    private var altitudeText: String {
        controller.currentAltitude.map { "\(Int($0))" } ?? "—"
    }
}
