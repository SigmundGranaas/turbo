import SwiftUI

/// A tiny elevation sparkline — a filled area under a polyline — used in the
/// paths list. Mirrors `Spark` in the design bundle.
public struct Spark: View {
    let data: [Double]
    let color: Color
    public init(data: [Double], color: Color) {
        self.data = data
        self.color = color
    }

    public var body: some View {
        GeometryReader { geo in
            let w = geo.size.width, h = geo.size.height
            let lo = data.min() ?? 0
            let hi = data.max() ?? 1
            let range = max(hi - lo, 0.0001)
            let step = data.count > 1 ? w / CGFloat(data.count - 1) : w
            let points = data.enumerated().map { i, v in
                CGPoint(x: CGFloat(i) * step, y: h - CGFloat((v - lo) / range) * h)
            }

            ZStack {
                // filled area
                Path { p in
                    guard let first = points.first else { return }
                    p.move(to: CGPoint(x: 0, y: h))
                    p.addLine(to: first)
                    for pt in points.dropFirst() { p.addLine(to: pt) }
                    p.addLine(to: CGPoint(x: w, y: h))
                    p.closeSubpath()
                }
                .fill(color.opacity(0.18))
                // line
                Path { p in
                    guard let first = points.first else { return }
                    p.move(to: first)
                    for pt in points.dropFirst() { p.addLine(to: pt) }
                }
                .stroke(color, style: StrokeStyle(lineWidth: 1.6, lineCap: .round, lineJoin: .round))
            }
        }
    }
}
