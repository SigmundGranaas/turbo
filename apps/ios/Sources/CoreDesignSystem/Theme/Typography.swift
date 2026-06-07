import SwiftUI

/// SF Pro type roles, mirroring the `TypeCard` in the design bundle
/// (`ios/iosFoundations.jsx`). These map onto the system text styles so Dynamic
/// Type keeps working; weights match the design (titles bold, headlines semibold).
public extension Font {
    /// Large Title · 34 / Bold
    static let turboLargeTitle = Font.system(.largeTitle, design: .default).weight(.bold)
    /// Title 1 · 28 / Bold
    static let turboTitle = Font.system(.title, design: .default).weight(.bold)
    /// Title 2 · 22 / Bold
    static let turboTitle2 = Font.system(.title2, design: .default).weight(.bold)
    /// Title 3 · 20 / Semibold
    static let turboTitle3 = Font.system(.title3, design: .default).weight(.semibold)
    /// Headline · 17 / Semibold
    static let turboHeadline = Font.system(.headline)
    /// Body · 17 / Regular
    static let turboBody = Font.system(.body)
    /// Subhead · 15 / Regular
    static let turboSubhead = Font.system(.subheadline)
    /// Footnote · 13 / Regular
    static let turboFootnote = Font.system(.footnote)
    /// Caption · 12 / Medium
    static let turboCaption = Font.system(.caption).weight(.medium)
}
