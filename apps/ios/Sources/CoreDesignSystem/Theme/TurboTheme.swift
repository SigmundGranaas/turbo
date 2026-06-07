import SwiftUI

/// The app theme container. Resolves ``TurboColors`` for the active color scheme,
/// injects them into the environment, and applies the system Blue tint.
///
/// Mirrors `TurboTheme { … }` in the Android app (which wraps content in the M3
/// `MaterialTheme`). Wrap the root view in this once.
public struct TurboTheme<Content: View>: View {
    @Environment(\.colorScheme) private var systemScheme
    private let forcedScheme: ColorScheme?
    private let content: Content

    /// - Parameter scheme: force light/dark, or `nil` to follow the system.
    public init(scheme: ColorScheme? = nil, @ViewBuilder content: () -> Content) {
        self.forcedScheme = scheme
        self.content = content()
    }

    public var body: some View {
        let scheme = forcedScheme ?? systemScheme
        let colors = TurboColors(dark: scheme == .dark)
        content
            .environment(\.turbo, colors)
            .tint(colors.tint)
            .preferredColorScheme(forcedScheme)
    }
}
