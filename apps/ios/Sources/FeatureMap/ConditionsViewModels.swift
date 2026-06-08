import Foundation
import Observation
import CoreModel
import CoreCommon
import CoreData

/// Loads a weather forecast for a place. Mirrors `feature.conditions.ConditionsViewModel`.
@MainActor
@Observable
public final class WeatherViewModel {
    public private(set) var state: LoadState<WeatherSummary> = .idle
    public private(set) var sun: SunTimes?
    public private(set) var marine: MarineConditions?
    private let provider: WeatherProvider
    private let reverseGeocode: ReverseGeocodeRepository?
    private let sunProvider: SunProvider?
    private let marineProvider: MarineProvider?
    private let position: LatLng
    private let fallbackName: String

    public init(provider: WeatherProvider, position: LatLng, placeName: String,
                reverseGeocode: ReverseGeocodeRepository? = nil,
                sunProvider: SunProvider? = nil,
                marineProvider: MarineProvider? = nil) {
        self.provider = provider
        self.reverseGeocode = reverseGeocode
        self.sunProvider = sunProvider
        self.marineProvider = marineProvider
        self.position = position
        self.fallbackName = placeName
    }

    public func load() async {
        state = .loading
        // Sun, marine, and place-name lookups are independent — run concurrently.
        async let sunTimes = sunProvider?.sun(at: position, date: Date())
        async let marineConditions = marineProvider?.conditions(at: position)
        let name = await reverseGeocode?.describe(position)?.label ?? fallbackName
        let summary = await provider.forecast(at: position, placeName: name)
        state = .resolve(summary, failure: "Couldn't load the forecast for this area.")
        sun = await sunTimes ?? nil
        marine = await marineConditions ?? nil
    }
}

/// Loads avalanche danger for a place. `nil` from the provider means no warning
/// is issued for the area (the common case off-season / outside forecast zones).
@MainActor
@Observable
public final class AvalancheViewModel {
    public private(set) var state: LoadState<AvalancheInfo> = .idle
    private let provider: AvalancheProvider
    private let position: LatLng

    public init(provider: AvalancheProvider, position: LatLng) {
        self.provider = provider
        self.position = position
    }

    public func load() async {
        state = .loading
        let info = await provider.danger(at: position)
        state = info.map(LoadState.loaded) ?? .empty
    }
}
