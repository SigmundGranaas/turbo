import SwiftUI

// Liquid Glass map chrome — the floating controls that sit over the full-bleed
// map. Ports the components in `ios/iosUI.jsx` + `ios/iosMap.jsx`.

// MARK: - Control rail

/// A vertical stack of round map controls on a single Liquid Glass surface.
/// Interleave ``MapRailButton``s with ``MapRailDivider`` for the hairline rule.
public struct MapControlRail<Content: View>: View {
    private let content: Content
    public init(@ViewBuilder content: () -> Content) { self.content = content() }

    public var body: some View {
        VStack(spacing: 0) { content }
            .frame(width: 48)   // keep the stack rail-width; dividers don't stretch it
            .liquidGlass(RoundedRectangle(cornerRadius: 24, style: .continuous))
    }
}

/// The 0.5pt hairline between rail buttons.
public struct MapRailDivider: View {
    @Environment(\.turbo) private var t
    public init() {}
    public var body: some View {
        Rectangle()
            .fill(t.glassHairline)
            .frame(height: 0.5)
            .padding(.horizontal, 9)
    }
}

/// A single round control in the rail (48×48). When `active`, it gets a tinted
/// halo + colored glyph — the design's strongest state.
public struct MapRailButton: View {
    @Environment(\.turbo) private var t
    let symbol: String
    let active: Bool
    let tint: Color?
    let action: () -> Void

    public init(symbol: String, active: Bool = false, tint: Color? = nil, action: @escaping () -> Void) {
        self.symbol = symbol
        self.active = active
        self.tint = tint
        self.action = action
    }

    public var body: some View {
        Button(action: action) {
            ZStack {
                if active {
                    Circle().fill((tint ?? t.blue).opacity(0.13)).padding(5)
                }
                Image(systemName: symbol)
                    .font(.system(size: 21, weight: .semibold))
                    .foregroundStyle(active ? (tint ?? t.blue) : t.label)
            }
            .frame(width: 48, height: 48)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
    }
}

// MARK: - FAB

/// The round primary action button — a solid tinted circle with a soft colored
/// lift. Mirrors `GlassFab`.
public struct MapFAB: View {
    @Environment(\.turbo) private var t
    let symbol: String
    let tint: Color?
    let size: CGFloat
    let action: () -> Void

    public init(symbol: String = "plus", tint: Color? = nil, size: CGFloat = 52, action: @escaping () -> Void) {
        self.symbol = symbol
        self.tint = tint
        self.size = size
        self.action = action
    }

    public var body: some View {
        let color = tint ?? t.blue
        Button(action: action) {
            Image(systemName: symbol)
                .font(.system(size: size * 0.46, weight: .semibold))
                .foregroundStyle(.white)
                .frame(width: size, height: size)
                .background(color, in: Circle())
                .shadow(color: color.opacity(0.4), radius: 9, x: 0, y: 6)
        }
        .buttonStyle(.plain)
    }
}

// MARK: - Weather chip

/// Top-left WeatherKit teaser — a glass pill with conditions + temperature.
public struct WeatherChip: View {
    @Environment(\.turbo) private var t
    let symbol: String
    let temperature: String
    public init(symbol: String = "cloud.snow.fill", temperature: String) {
        self.symbol = symbol
        self.temperature = temperature
    }
    public var body: some View {
        HStack(spacing: 6) {
            Image(systemName: symbol)
                .font(.system(size: 16, weight: .semibold))
                .foregroundStyle(t.blue)
            Text(temperature)
                .font(.system(size: 15, weight: .semibold))
                .foregroundStyle(t.label)
        }
        .padding(.vertical, 7)
        .padding(.horizontal, 11)
        .liquidGlass(Capsule())
    }
}

// MARK: - Search pill

/// The bottom glass search bar. Tapping it opens search (a button, not a field).
public struct SearchPill: View {
    @Environment(\.turbo) private var t
    let placeholder: String
    let action: () -> Void
    public init(placeholder: String = "Search Turbo", action: @escaping () -> Void) {
        self.placeholder = placeholder
        self.action = action
    }
    public var body: some View {
        Button(action: action) {
            HStack(spacing: 9) {
                Image(systemName: "magnifyingglass")
                    .font(.system(size: 18, weight: .semibold))
                    .foregroundStyle(t.label2)
                Text(placeholder)
                    .font(.system(size: 17))
                    .foregroundStyle(t.label2)
                Spacer(minLength: 0)
            }
            .padding(.horizontal, 16)
            .frame(height: 52)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .liquidGlass(Capsule())
        .accessibilityLabel("Search places and trails")
    }
}

// MARK: - Compass

/// The compass needle — red north, gray south — rotating opposite the heading.
/// Mirrors `Compass` in `ios/iosUI.jsx`.
public struct CompassDial: View {
    @Environment(\.turbo) private var t
    let heading: Double
    let size: CGFloat
    public init(heading: Double, size: CGFloat = 24) {
        self.heading = heading
        self.size = size
    }
    public var body: some View {
        Canvas { context, canvasSize in
            let w = canvasSize.width, h = canvasSize.height
            let cx = w / 2, cy = h / 2
            // A clean two-triangle needle: red north, gray south, meeting at a hub.
            var north = Path()
            north.move(to: CGPoint(x: cx, y: h * 0.12))
            north.addLine(to: CGPoint(x: w * 0.33, y: cy))
            north.addLine(to: CGPoint(x: w * 0.67, y: cy))
            north.closeSubpath()
            context.fill(north, with: .color(t.red))

            var south = Path()
            south.move(to: CGPoint(x: cx, y: h * 0.88))
            south.addLine(to: CGPoint(x: w * 0.33, y: cy))
            south.addLine(to: CGPoint(x: w * 0.67, y: cy))
            south.closeSubpath()
            context.fill(south, with: .color(t.label2))

            // Center hub keeps the two halves visually joined.
            let hub = CGRect(x: cx - w * 0.06, y: cy - h * 0.06, width: w * 0.12, height: h * 0.12)
            context.fill(Path(ellipseIn: hub), with: .color(t.label))
        }
        .frame(width: size, height: size)
        .rotationEffect(.degrees(-heading))
    }
}

// MARK: - Scale bar

/// The bottom-left scale bar, drawn over the map (white with a drop shadow so it
/// reads on any tile).
public struct ScaleBar: View {
    let label: String
    let width: CGFloat
    public init(label: String = "500 m", width: CGFloat = 56) {
        self.label = label
        self.width = width
    }
    public var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(label)
                .font(.system(size: 11, weight: .semibold))
                .foregroundStyle(.white)
            Rectangle()
                .fill(.white)
                .frame(width: width, height: 3)
                .clipShape(RoundedRectangle(cornerRadius: 2))
        }
        .shadow(color: .black.opacity(0.5), radius: 2, x: 0, y: 1)
    }
}

// MARK: - Account avatar

/// Top-right account button — a glass circle wrapping a gradient monogram when
/// signed in, or a generic person glyph when there's no account.
public struct MapAvatar: View {
    @Environment(\.turbo) private var t
    let initials: String?
    let action: () -> Void
    public init(initials: String?, action: @escaping () -> Void) {
        self.initials = initials
        self.action = action
    }
    public var body: some View {
        Button(action: action) {
            Group {
                if let initials {
                    Monogram(initials: initials, size: 30)
                } else {
                    Image(systemName: "person.crop.circle.fill")
                        .font(.system(size: 28))
                        .foregroundStyle(t.label2)
                        .frame(width: 30, height: 30)
                }
            }
            .padding(4)
            .contentShape(Circle())
        }
        .buttonStyle(.plain)
        .liquidGlass(Circle())
    }
}
