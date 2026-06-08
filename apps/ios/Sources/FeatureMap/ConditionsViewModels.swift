import Foundation
import Observation
import CoreModel
import CoreData

/// Loads a weather forecast for a place. Mirrors `feature.conditions.ConditionsViewModel`.
@MainActor
@Observable
public final class WeatherViewModel {
    public private(set) var summary: WeatherSummary?
    public private(set) var sun: SunTimes?
    public private(set) var loaded = false
    private let provider: WeatherProvider
    private let reverseGeocode: ReverseGeocodeRepository?
    private let sunProvider: SunProvider?
    private let position: LatLng
    private let fallbackName: String

    public init(provider: WeatherProvider, position: LatLng, placeName: String,
                reverseGeocode: ReverseGeocodeRepository? = nil,
                sunProvider: SunProvider? = nil) {
        self.provider = provider
        self.reverseGeocode = reverseGeocode
        self.sunProvider = sunProvider
        self.position = position
        self.fallbackName = placeName
    }

    public func load() async {
        // Sun + place-name lookups are independent — run them concurrently.
        async let sunTimes = sunProvider?.sun(at: position, date: Date())
        let name = await reverseGeocode?.describe(position)?.label ?? fallbackName
        summary = await provider.forecast(at: position, placeName: name)
        sun = await sunTimes ?? nil
        loaded = true
    }
}

/// Loads avalanche danger for a place.
@MainActor
@Observable
public final class AvalancheViewModel {
    public private(set) var info: AvalancheInfo?
    public private(set) var loaded = false
    private let provider: AvalancheProvider
    private let position: LatLng

    public init(provider: AvalancheProvider, position: LatLng) {
        self.provider = provider
        self.position = position
    }

    public func load() async {
        info = await provider.danger(at: position)
        loaded = true
    }
}
