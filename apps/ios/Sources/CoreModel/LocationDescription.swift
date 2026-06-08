import Foundation

/// How a place name relates to the queried point. Mirrors `domain.PlaceQualifier`.
public enum PlaceQualifier: Sendable {
    case on, `in`, near, at
}

/// A reverse-geocoded description of a point — nearest place name + qualifier.
/// Mirrors `domain.LocationDescription`.
public struct LocationDescription: Equatable, Sendable {
    public let title: String
    public let qualifier: PlaceQualifier?
    public let secondary: String?
    public let elevationM: Double?

    public init(title: String, qualifier: PlaceQualifier? = nil, secondary: String? = nil, elevationM: Double? = nil) {
        self.title = title
        self.qualifier = qualifier
        self.secondary = secondary
        self.elevationM = elevationM
    }

    /// Headline, e.g. "On Galdhøpiggen" / "In Lom".
    public var label: String {
        switch qualifier {
        case .on: "On \(title)"
        case .in: "In \(title)"
        case .near: "Near \(title)"
        case .at, nil: title
        }
    }

    /// Supporting line, e.g. "fjelltopp · 2469 m".
    public var subtitle: String {
        [secondary?.nonBlank, elevationM.map { "\(Int($0)) m" }]
            .compactMap { $0 }
            .joined(separator: " · ")
    }
}

private extension String {
    var nonBlank: String? { trimmingCharacters(in: .whitespaces).isEmpty ? nil : self }
}
