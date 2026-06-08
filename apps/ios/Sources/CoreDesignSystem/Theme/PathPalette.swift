import SwiftUI
import CoreModel

public extension Color {
    /// Build a `Color` from a packed `0xAARRGGBB` value (matches `Marker.colorArgb`).
    init(argb: Int64) {
        let a = Double((argb >> 24) & 0xFF) / 255.0
        let r = Double((argb >> 16) & 0xFF) / 255.0
        let g = Double((argb >> 8) & 0xFF) / 255.0
        let b = Double(argb & 0xFF) / 255.0
        self.init(.sRGB, red: r, green: g, blue: b, opacity: a == 0 ? 1 : a)
    }
}

/// The seven hand-tuned waypoint colours users can tint markers + paths with —
/// the `pathColorPalette` from the design system.
public enum PathPalette {
    /// `(name, packed ARGB)` swatches.
    public static let swatches: [(name: String, argb: Int64)] = [
        ("Blue", 0xFF1976D2),
        ("Red", 0xFFD32F2F),
        ("Green", 0xFF388E3C),
        ("Orange", 0xFFF57C00),
        ("Purple", 0xFF7B1FA2),
        ("Teal", 0xFF00897B),
        ("Pink", 0xFFC2185B),
    ]
}

public extension Marker {
    /// The marker's display tint: its custom colour override, else the kind's tint.
    func displayColor(_ t: TurboColors) -> Color {
        colorArgb.map { Color(argb: $0) } ?? kind.tint(t)
    }
}
