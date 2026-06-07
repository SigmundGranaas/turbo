import SwiftUI

extension Color {
    /// Build a `Color` from a `0xRRGGBB` / `0xRRGGBBAA` hex literal.
    /// Used by ``TurboColors`` to mirror the design system's exact hex tokens.
    public init(hex: UInt32, alpha: Double = 1.0) {
        let r = Double((hex >> 16) & 0xFF) / 255.0
        let g = Double((hex >> 8) & 0xFF) / 255.0
        let b = Double(hex & 0xFF) / 255.0
        self.init(.sRGB, red: r, green: g, blue: b, opacity: alpha)
    }
}
