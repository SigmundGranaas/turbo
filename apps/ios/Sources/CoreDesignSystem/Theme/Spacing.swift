import SwiftUI

/// Spacing, corner-radius and motion tokens. Names mirror the design system's
/// CSS variables (`colors_and_type.css`) so values stay traceable to source.
public enum Space {
    public static let xxs: CGFloat = 2
    public static let xs: CGFloat = 4
    public static let s: CGFloat = 8
    public static let m: CGFloat = 12
    public static let l: CGFloat = 16   // the system's heartbeat
    public static let xl: CGFloat = 24
    public static let xxl: CGFloat = 32
    public static let xxxl: CGFloat = 48
}

public enum Radius {
    public static let s: CGFloat = 8
    public static let m: CGFloat = 12
    public static let l: CGFloat = 16
    public static let xl: CGFloat = 20
    /// Inset-grouped card radius used across the iOS screens.
    public static let card: CGFloat = 14
    public static let xxl: CGFloat = 24
    public static let control: CGFloat = 26  // glass pill controls
}

public enum Motion {
    /// Map camera moves — 300ms easeOut. Mirrors the Android tween.
    public static let base: Animation = .easeOut(duration: 0.30)
    /// Compass rotations — 200ms.
    public static let fast: Animation = .easeOut(duration: 0.20)
    /// Compass reset — 500ms emphasized.
    public static let slow: Animation = .timingCurve(0.4, 0, 0.2, 1, duration: 0.50)
}
