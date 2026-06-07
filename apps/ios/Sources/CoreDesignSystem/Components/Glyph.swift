import SwiftUI

/// An SF Symbol in a tinted rounded square — the iOS settings-list leading
/// glyph. Mirrors the `Glyph` component in `ios/iosUI.jsx`.
public struct Glyph: View {
    let symbol: String
    let color: Color
    let size: CGFloat
    let cornerRadius: CGFloat?

    public init(symbol: String, color: Color, size: CGFloat = 30, cornerRadius: CGFloat? = nil) {
        self.symbol = symbol
        self.color = color
        self.size = size
        self.cornerRadius = cornerRadius
    }

    public var body: some View {
        RoundedRectangle(cornerRadius: cornerRadius ?? size * 0.29, style: .continuous)
            .fill(color)
            .frame(width: size, height: size)
            .overlay(
                Image(systemName: symbol)
                    .font(.system(size: size * 0.52, weight: .semibold))
                    .foregroundStyle(.white)
            )
    }
}

#Preview {
    HStack(spacing: 12) {
        Glyph(symbol: "mountain.2.fill", color: .green, size: 38, cornerRadius: 10)
        Glyph(symbol: "arrow.down", color: .blue, size: 38, cornerRadius: 10)
        Glyph(symbol: "checkmark", color: .green, size: 38, cornerRadius: 10)
    }
    .padding()
}
