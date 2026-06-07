import SwiftUI

/// The signature **Liquid Glass** surface — translucent floating controls that
/// refract the live map beneath them. Mirrors the `glass(t, …)` helper in the
/// design bundle (`ios/iosKit.jsx`).
///
/// On iOS 26 this uses the native `.glassEffect`; older OSes fall back to a
/// `.ultraThinMaterial` fill with the design's hairline border + lift shadow,
/// so the same call site works everywhere.
public struct LiquidGlassModifier: ViewModifier {
    @Environment(\.turbo) private var turbo
    let shape: AnyShape
    let elevated: Bool

    public func body(content: Content) -> some View {
        if #available(iOS 26.0, macOS 26.0, *) {
            content.glassEffect(.regular, in: shape)
        } else {
            content
                .background(.ultraThinMaterial, in: shape)
                .overlay(shape.strokeBorder(turbo.glassBorder, lineWidth: 0.5))
                .shadow(
                    color: .black.opacity(turbo.dark ? 0.5 : 0.14),
                    radius: elevated ? 15 : 11,
                    x: 0,
                    y: elevated ? 8 : 6
                )
        }
    }
}

public extension View {
    /// Apply the Liquid Glass material, clipped to `shape`.
    /// - Parameters:
    ///   - shape: clipping shape (default: capsule, the iOS control default).
    ///   - elevated: use a slightly more opaque, higher-lift variant for sheets.
    func liquidGlass(
        _ shape: some Shape = Capsule(),
        elevated: Bool = false
    ) -> some View {
        modifier(LiquidGlassModifier(shape: AnyShape(shape), elevated: elevated))
    }

    /// Liquid Glass with a rounded rectangle of the given corner radius.
    func liquidGlass(cornerRadius: CGFloat, elevated: Bool = false) -> some View {
        liquidGlass(RoundedRectangle(cornerRadius: cornerRadius, style: .continuous), elevated: elevated)
    }
}

private extension Shape {
    /// `strokeBorder` is only on `InsettableShape`; this keeps the fallback border
    /// drawable for the `AnyShape`-wrapped clip shape.
    func strokeBorder(_ color: Color, lineWidth: CGFloat) -> some View {
        stroke(color, lineWidth: lineWidth)
    }
}
