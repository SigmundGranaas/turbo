import SwiftUI

/// The iOS equivalent of Android's `LinearWavyProgressIndicator` (US-2): the covered/tracked
/// portion renders as a slow, calm sine wave, the road ahead stays flat — so it reads as a
/// progress tracker, not a spinner. `value` is the route fraction (0…1).
struct WavyProgressView: View {
    var value: Double
    var tint: Color
    /// A gentle, near-static wave: one full traverse every 7 s.
    private let period: Double = 7

    @State private var phase: CGFloat = 0

    var body: some View {
        GeometryReader { geo in
            let w = geo.size.width
            let filled = w * CGFloat(min(max(value, 0), 1))
            ZStack(alignment: .leading) {
                Capsule()
                    .fill(tint.opacity(0.18))
                    .frame(height: 4)
                    .frame(maxHeight: .infinity, alignment: .center)
                WavyShape(phase: phase, amplitude: 2.6, wavelength: 16, width: filled)
                    .stroke(tint, style: StrokeStyle(lineWidth: 4, lineCap: .round, lineJoin: .round))
            }
        }
        .frame(height: 14)
        .onAppear {
            withAnimation(.linear(duration: period).repeatForever(autoreverses: false)) {
                phase = .pi * 2
            }
        }
    }
}

/// A horizontal sine wave from x=0 to `width`, animated by `phase`.
private struct WavyShape: Shape {
    var phase: CGFloat
    let amplitude: CGFloat
    let wavelength: CGFloat
    let width: CGFloat

    var animatableData: CGFloat {
        get { phase }
        set { phase = newValue }
    }

    func path(in rect: CGRect) -> Path {
        var p = Path()
        let midY = rect.midY
        guard width > 0 else { return p }
        p.move(to: CGPoint(x: 0, y: midY))
        var x: CGFloat = 0
        while x <= width {
            let y = midY + amplitude * sin((x / wavelength) * .pi * 2 + phase)
            p.addLine(to: CGPoint(x: x, y: y))
            x += 1.5
        }
        return p
    }
}
