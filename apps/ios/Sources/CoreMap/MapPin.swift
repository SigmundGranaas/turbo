import SwiftUI
import CoreModel

/// A render-ready map annotation: where to drop the pin, which SF Symbol to show,
/// and its tint. Built by the feature layer (which owns the design-system
/// activity visuals) and handed to ``TurboMapView`` — this keeps `CoreMap` free
/// of any dependency on `CoreDesignSystem`.
public struct MapPin: Identifiable, Equatable {
    public let id: String
    public let coordinate: LatLng
    public let title: String
    public let symbolName: String
    public let tint: Color

    public init(id: String, coordinate: LatLng, title: String, symbolName: String, tint: Color) {
        self.id = id
        self.coordinate = coordinate
        self.title = title
        self.symbolName = symbolName
        self.tint = tint
    }
}
