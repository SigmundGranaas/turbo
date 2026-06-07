import Foundation
import Observation
import CoreModel
import CoreData

/// Loads a weather forecast for a place. Mirrors `feature.conditions.ConditionsViewModel`.
@MainActor
@Observable
public final class WeatherViewModel {
    public private(set) var summary: WeatherSummary?
    private let provider: WeatherProvider
    private let position: LatLng
    private let placeName: String

    public init(provider: WeatherProvider, position: LatLng, placeName: String) {
        self.provider = provider
        self.position = position
        self.placeName = placeName
    }

    public func load() async {
        summary = await provider.forecast(at: position, placeName: placeName)
    }
}

/// Loads avalanche danger for a place.
@MainActor
@Observable
public final class AvalancheViewModel {
    public private(set) var info: AvalancheInfo?
    private let provider: AvalancheProvider
    private let position: LatLng

    public init(provider: AvalancheProvider, position: LatLng) {
        self.provider = provider
        self.position = position
    }

    public func load() async {
        info = await provider.danger(at: position)
    }
}
