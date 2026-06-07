import Foundation
import SwiftData
import CoreModel
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
    public let syncController: SyncController

    /// Production wiring — SwiftData persistence + UserDefaults settings. Falls
    /// back to in-memory if the store can't be opened (e.g. previews).
    public init() {
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
        authRepository = InMemoryAuthRepository()
        offlineManager = DiskOfflineTileManager()
        syncController = SyncController(
            engine: MarkerSyncEngine(repository: markerRepository, transport: InMemoryMarkerSyncTransport()),
            auth: authRepository,
            settings: settingsRepository
        )
    }

    /// Explicit injection — for previews and tests.
    public init(
        markerRepository: MarkerRepository,
        settingsRepository: SettingsRepository = InMemorySettingsRepository(),
        searchRepository: SearchRepository = InMemorySearchRepository(),
        pathRepository: PathRepository = InMemoryPathRepository(),
        collectionRepository: CollectionRepository = InMemoryCollectionRepository(),
        authRepository: AuthRepository = InMemoryAuthRepository(),
        offlineManager: OfflineTileManager = InMemoryOfflineTileManager()
    ) {
        self.markerRepository = markerRepository
        self.settingsRepository = settingsRepository
        self.searchRepository = searchRepository
        self.pathRepository = pathRepository
        self.collectionRepository = collectionRepository
        self.authRepository = authRepository
        self.offlineManager = offlineManager
        self.syncController = SyncController(
            engine: MarkerSyncEngine(repository: markerRepository, transport: InMemoryMarkerSyncTransport()),
            auth: authRepository,
            settings: settingsRepository
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
            return AppContainer(
                markerRepository: InMemoryMarkerRepository(),
                searchRepository: InMemorySearchRepository(),
                pathRepository: InMemoryPathRepository(),
                collectionRepository: InMemoryCollectionRepository(),
                authRepository: InMemoryAuthRepository(),
                offlineManager: InMemoryOfflineTileManager()
            )
        }
        return AppContainer()
    }

    // MARK: View-model factories

    public func makeMapViewModel() -> MapViewModel { MapViewModel(markerRepository: markerRepository) }
    public func makeMarkersViewModel() -> MarkersViewModel { MarkersViewModel(repository: markerRepository) }
    public func makeSearchViewModel() -> SearchViewModel { SearchViewModel(repository: searchRepository) }
    public func makeSettingsViewModel() -> SettingsViewModel { SettingsViewModel(repository: settingsRepository) }
    public func makePathsViewModel() -> PathsViewModel { PathsViewModel(repository: pathRepository) }
    public func makeCollectionsViewModel() -> CollectionsViewModel { CollectionsViewModel(repository: collectionRepository) }
    public func makeAuthViewModel() -> AuthViewModel { AuthViewModel(repository: authRepository) }
    public func makeOfflineViewModel() -> OfflineViewModel { OfflineViewModel(manager: offlineManager) }
}
