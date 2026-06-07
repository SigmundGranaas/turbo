import SwiftUI

/// The Turbo · iOS 26 color tokens — a direct port of `iosTheme(dark)` from the
/// design bundle (`ios/iosKit.jsx`). Apple system colors over adaptive neutrals,
/// Blue tint, plus the translucent "Liquid Glass" surface fills.
///
/// Resolved once per `ColorScheme` and handed down through the SwiftUI
/// environment as ``EnvironmentValues/turbo``. Reach for these instead of raw
/// `Color` literals so light/dark stay in lockstep with the design.
public struct TurboColors: Sendable {
    public let dark: Bool

    // System accent palette
    public let blue: Color
    public let green: Color
    public let red: Color
    public let orange: Color
    public let yellow: Color
    public let teal: Color
    public let indigo: Color
    public let purple: Color
    public let pink: Color
    /// Default app tint (== blue).
    public var tint: Color { blue }

    // Backgrounds
    public let background: Color       // bg
    public let secondaryBackground: Color // bg2
    public let grouped: Color          // grouped page background
    public let groupedCard: Color      // grouped2 — inset card fill
    public let elevated: Color

    // Labels
    public let label: Color
    public let label2: Color
    public let label3: Color

    // Separators & fills
    public let separator: Color        // sep
    public let fill: Color
    public let fill2: Color
    public let fill3: Color
    public let gray: Color

    // Liquid Glass surface fills (used by `liquidGlass` fallback + tints)
    public let glassBackground: Color
    public let glassBackgroundHigh: Color
    public let glassBorder: Color
    public let glassHairline: Color

    public init(dark: Bool) {
        self.dark = dark
        blue   = Color(hex: dark ? 0x0A84FF : 0x007AFF)
        green  = Color(hex: dark ? 0x30D158 : 0x34C759)
        red    = Color(hex: dark ? 0xFF453A : 0xFF3B30)
        orange = Color(hex: dark ? 0xFF9F0A : 0xFF9500)
        yellow = Color(hex: dark ? 0xFFD60A : 0xFFCC00)
        teal   = Color(hex: dark ? 0x40CBE0 : 0x30B0C7)
        indigo = Color(hex: dark ? 0x5E5CE6 : 0x5856D6)
        purple = Color(hex: dark ? 0xBF5AF2 : 0xAF52DE)
        pink   = Color(hex: dark ? 0xFF375F : 0xFF2D55)

        background          = Color(hex: dark ? 0x000000 : 0xFFFFFF)
        secondaryBackground = Color(hex: dark ? 0x1C1C1E : 0xF2F2F7)
        grouped             = Color(hex: dark ? 0x000000 : 0xF2F2F7)
        groupedCard         = Color(hex: dark ? 0x1C1C1E : 0xFFFFFF)
        elevated            = Color(hex: dark ? 0x1C1C1E : 0xFFFFFF)

        label  = Color(hex: dark ? 0xFFFFFF : 0x000000)
        label2 = Color(hex: dark ? 0xEBEBF5 : 0x3C3C43, alpha: 0.6)
        label3 = Color(hex: dark ? 0xEBEBF5 : 0x3C3C43, alpha: 0.3)

        separator = Color(hex: dark ? 0x545458 : 0x3C3C43, alpha: dark ? 0.65 : 0.29)
        fill  = Color(hex: 0x787880, alpha: dark ? 0.36 : 0.20)
        fill2 = Color(hex: 0x787880, alpha: dark ? 0.32 : 0.16)
        fill3 = Color(hex: 0x767680, alpha: dark ? 0.24 : 0.12)
        gray  = Color(hex: 0x8E8E93)

        glassBackground     = Color(hex: dark ? 0x1E1E20 : 0xFFFFFF, alpha: dark ? 0.62 : 0.62)
        glassBackgroundHigh = Color(hex: dark ? 0x2C2C30 : 0xFFFFFF, alpha: dark ? 0.74 : 0.78)
        glassBorder         = Color(hex: dark ? 0xFFFFFF : 0xFFFFFF, alpha: dark ? 0.12 : 0.60)
        glassHairline       = Color(hex: dark ? 0xFFFFFF : 0x000000, alpha: dark ? 0.08 : 0.05)
    }
}

// MARK: - Environment plumbing

private struct TurboColorsKey: EnvironmentKey {
    static let defaultValue = TurboColors(dark: false)
}

public extension EnvironmentValues {
    /// The resolved Turbo color tokens for the current color scheme.
    var turbo: TurboColors {
        get { self[TurboColorsKey.self] }
        set { self[TurboColorsKey.self] = newValue }
    }
}
