import SwiftUI

/// A gradient avatar monogram (initials on a blue gradient) — the account
/// identity used in the menu, settings and profile. Mirrors `Avatar` in the
/// design bundle.
public struct Monogram: View {
    let initials: String
    let size: CGFloat
    public init(initials: String, size: CGFloat = 58) {
        self.initials = initials
        self.size = size
    }
    public var body: some View {
        Text(initials)
            .font(.system(size: size * 0.42, weight: .semibold))
            .foregroundStyle(.white)
            .frame(width: size, height: size)
            .background(
                LinearGradient(
                    colors: [Color(hex: 0x5AC8FA), Color(hex: 0x0A84FF)],
                    startPoint: .topLeading, endPoint: .bottomTrailing
                ),
                in: Circle()
            )
    }
}
