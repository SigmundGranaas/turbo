import Foundation
import SwiftData
import CoreModel
import CoreCommon
import CoreData
import CoreAuth
import CoreSync
import CoreMap
import FeatureMap
import FeatureSearch
import FeatureSettings
import FeatureRecording
import FeatureAuth
import FeatureCollections
import FeatureOffline

/// The composition root — constructs the singletons (repositories, seams) and
/// vends view models with their dependencies injected. The hand-rolled
/// equivalent of Android's Hilt graph: one place that owns wiring, so feature
/// modules stay free of construction logic.
@MainActor
public final class AppContainer {

    // MARK: Singletons (seams / repositories)

    public let markerRepository: MarkerRepository
    public let settingsRepository: SettingsRepository
    public let searchRepository: SearchRepository
    public let pathRepository: PathRepository
    public let collectionRepository: CollectionRepository
    public let authRepository: AuthRepository
    public let offlineManager: OfflineTileManager
    public let locationProvider: LocationProvider
    public let weatherProvider: WeatherProvider
    public let avalancheProvider: AvalancheProvider
    public let syncController: SyncController
    /// Whether the live API (auth + cloud sync) is configured for this build.
    public let isOnline: Bool

    /// Production wiring — SwiftData persistence + UserDefaults settings. With a
    /// configured `TurboConfig.apiBaseURL` it uses the live Google auth + HTTP
    /// sync; otherwise it stays fully local. Falls back to in-memory persistence
    /// if the store can't be opened (e.g. previews).
    public init(config: TurboConfig = .fromBundle()) {
        let container = try? TurboPersistence.container()
        if let container {
            markerRepository = SwiftDataMarkerRepository(container: container)
            pathRepository = SwiftDataPathRepository(container: container)
            collectionRepository = SwiftDataCollectionRepository(container: container)
        } else {
            markerRepository = InMemoryMarkerRepository(seed: [])
            pathRepository = InMemoryPathRepository(seed: [])
            collectionRepository = InMemoryCollectionRepository(seed: [])
        }
        settingsRepository = UserDefaultsSettingsRepository()
        searchRepository = KartverketSearchRepository()
        offlineManager = DiskOfflineTileManager()
        locationProvider = CoreLocationProvider()
        weatherProvider = InMemoryWeatherProvider()
        avalancheProvider = InMemoryAvalancheProvider()
        isOnline = config.isOnline

        let cursor = UserDefaultsCursorStore()
        if let base = config.apiBaseURL {
            // Live: Google sign-in + HTTP sync against the API.
            let auth = GoogleAuthRepository(apiBaseURL: base)
            authRepository = auth
            syncController = SyncController(
                units: [
                    Syncers.marker(repository: markerRepository,
                                   transport: HttpSyncTransport<MarkerPayload>(endpoint: base.appendingPathComponent("markers"), token: { nil }),
                                   cursor: cursor),
                    Syncers.path(repository: pathRepository,
                                 transport: HttpSyncTransport<PathPayload>(endpoint: base.appendingPathComponent("paths"), token: { nil }),
                                 cursor: cursor),
                    Syncers.collection(repository: collectionRepository,
                                       transport: HttpSyncTransport<CollectionPayload>(endpoint: base.appendingPathComponent("collections"), token: { nil }),
                                       cursor: cursor),
                ],
                auth: authRepository, settings: settingsRepository
            )
        } else {
            // Local: in-memory auth + sync transports.
            authRepository = InMemoryAuthRepository()
            syncController = AppContainer.makeSyncController(
                markers: markerRepository, paths: pathRepository, collections: collectionRepository,
                auth: authRepository, settings: settingsRepository, cursor: cursor
            )
        }
    }

    /// Explicit injection — for previews and tests.
    public init(
        markerRepository: MarkerRepository,
        settingsRepository: SettingsRepository = InMemorySettingsRepository(),
        searchRepository: SearchRepository = InMemorySearchRepository(),
        pathRepository: PathRepository = InMemoryPathRepository(),
        collectionRepository: CollectionRepository = InMemoryCollectionRepository(),
        authRepository: AuthRepository = InMemoryAuthRepository(),
        offlineManager: OfflineTileManager = InMemoryOfflineTileManager(),
        locationProvider: LocationProvider = SimulatedLocationProvider(fixes: []),
        weatherProvider: WeatherProvider = InMemoryWeatherProvider(),
        avalancheProvider: AvalancheProvider = InMemoryAvalancheProvider()
    ) {
        self.markerRepository = markerRepository
        self.settingsRepository = settingsRepository
        self.searchRepository = searchRepository
        self.pathRepository = pathRepository
        self.collectionRepository = collectionRepository
        self.authRepository = authRepository
        self.offlineManager = offlineManager
        self.locationProvider = locationProvider
        self.weatherProvider = weatherProvider
        self.avalancheProvider = avalancheProvider
        self.isOnline = false
        self.syncController = AppContainer.makeSyncController(
            markers: markerRepository, paths: pathRepository, collections: collectionRepository,
            auth: authRepository, settings: settingsRepository, cursor: InMemoryCursorStore()
        )
    }

    /// Build the sync controller with one engine per entity, all on in-memory
    /// transports by default (real HTTP transports land once the API is configured).
    private static func makeSyncController(
        markers: MarkerRepository, paths: PathRepository, collections: CollectionRepository,
        auth: AuthRepository, settings: SettingsRepository, cursor: SyncCursorStore
    ) -> SyncController {
        SyncController(
            units: [
                Syncers.marker(repository: markers, transport: InMemorySyncTransport<MarkerPayload>(), cursor: cursor),
                Syncers.path(repository: paths, transport: InMemorySyncTransport<PathPayload>(), cursor: cursor),
                Syncers.collection(repository: collections, transport: InMemorySyncTransport<CollectionPayload>(), cursor: cursor),
            ],
            auth: auth, settings: settings
        )
    }

    /// Seed first-run sample content into empty persistent stores so a fresh
    /// install isn't blank. Idempotent — only fills empty repositories.
    public func seedIfEmpty() async {
        if await markerRepository.current().isEmpty {
            for marker in InMemoryMarkerRepository.sample { await markerRepository.upsert(marker) }
        }
        if await pathRepository.current().isEmpty {
            for path in InMemoryPathRepository.sample { await pathRepository.upsert(path) }
        }
        if await collectionRepository.current().isEmpty {
            for collection in InMemoryCollectionRepository.sample { await collectionRepository.upsert(collection) }
        }
    }

    /// Choose the container for this launch. With `-uitest` the app runs on
    /// deterministic, seeded **in-memory** backends (no network, no OAuth) so the
    /// end-to-end UI suite is hermetic; otherwise it uses the live backends.
    public static func resolve(arguments: [String] = ProcessInfo.processInfo.arguments) -> AppContainer {
        if arguments.contains("-uitest") {
            // Scripted location so recording captures a track deterministically.
            let fixes = (0..<6).map { i in
                LocationFix(position: LatLng(lat: 69.60 + Double(i) * 0.003, lng: 19.90 + Double(i) * 0.004),
                            headingDegrees: 45, altitude: 10 + Double(i) * 5)
            }
            return AppContainer(
                markerRepository: InMemoryMarkerRepository(),
                searchRepository: InMemorySearchRepository(),
                pathRepository: InMemoryPathRepository(),
                collectionRepository: InMemoryCollectionRepository(),
                authRepository: InMemoryAuthRepository(),
                offlineManager: InMemoryOfflineTileManager(),
                locationProvider: SimulatedLocationProvider(fixes: fixes, interval: .milliseconds(80))
            )
        }
        return AppContainer()
    }

    // MARK: View-model factories

    public func makeMapViewModel() -> MapViewModel {
        MapViewModel(markerRepository: markerRepository, location: locationProvider)
    }
    public func makeMarkersViewModel() -> MarkersViewModel { MarkersViewModel(repository: markerRepository) }
    public func makeSearchViewModel() -> SearchViewModel { SearchViewModel(repository: searchRepository) }
    public func makeSettingsViewModel() -> SettingsViewModel { SettingsViewModel(repository: settingsRepository) }
    public func makePathsViewModel() -> PathsViewModel { PathsViewModel(repository: pathRepository) }
    public func makeRecordingViewModel() -> RecordingViewModel {
        RecordingViewModel(location: locationProvider, pathRepository: pathRepository)
    }
    public func makeWeatherViewModel(position: LatLng, placeName: String) -> WeatherViewModel {
        WeatherViewModel(provider: weatherProvider, position: position, placeName: placeName)
    }
    public func makeAvalancheViewModel(position: LatLng) -> AvalancheViewModel {
        AvalancheViewModel(provider: avalancheProvider, position: position)
    }
    public func makeCollectionsViewModel() -> CollectionsViewModel { CollectionsViewModel(repository: collectionRepository) }
    public func makeAuthViewModel() -> AuthViewModel { AuthViewModel(repository: authRepository) }
    public func makeOfflineViewModel() -> OfflineViewModel { OfflineViewModel(manager: offlineManager) }
}
