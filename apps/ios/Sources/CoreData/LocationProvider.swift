import Foundation
import CoreModel
#if canImport(CoreLocation)
import CoreLocation
#endif

/// A single location reading — where the user is and (optionally) which way the
/// device is pointing. Mirrors what Android's `LocationRepository` streams.
public struct LocationFix: Equatable, Sendable {
    public let position: LatLng
    /// Compass heading in degrees (0 = north), when available.
    public let headingDegrees: Double?
    /// Altitude in metres, when available (drives recorded elevation profiles).
    public let altitude: Double?
    /// Ground speed in metres/second, when available (negative readings → nil).
    public let speedMps: Double?

    public init(position: LatLng, headingDegrees: Double? = nil, altitude: Double? = nil, speedMps: Double? = nil) {
        self.position = position
        self.headingDegrees = headingDegrees
        self.altitude = altitude
        self.speedMps = speedMps
    }
}

/// Streams the user's location + heading. The seam the map uses to follow the
/// hiker and orient the compass; the only place `CoreLocation` is touched.
public protocol LocationProvider: Sendable {
    /// Ask the user for "when in use" permission (no-op if already decided).
    func requestAuthorization()
    /// Ask for "always" permission so a recording survives backgrounding /
    /// screen-lock (the iOS analogue of Android's location foreground service).
    func requestAlwaysAuthorization()
    /// Keep delivering fixes while backgrounded. Enabled for the duration of a
    /// recording, disabled when it stops so the app isn't a battery drain at rest.
    func setBackgroundUpdates(_ enabled: Bool)
    /// A stream of location fixes for the lifetime of the subscription.
    func fixes() -> AsyncStream<LocationFix>
}

public extension LocationProvider {
    // Simulated / in-memory providers don't manage real CoreLocation state.
    func requestAlwaysAuthorization() {}
    func setBackgroundUpdates(_ enabled: Bool) {}
}

/// A scripted provider for tests + previews — emits a fixed sequence of fixes.
public final class SimulatedLocationProvider: LocationProvider {
    private let scripted: [LocationFix]
    private let interval: Duration

    public init(fixes: [LocationFix], interval: Duration = .milliseconds(20)) {
        self.scripted = fixes
        self.interval = interval
    }

    public func requestAuthorization() {}

    public func fixes() -> AsyncStream<LocationFix> {
        AsyncStream { continuation in
            let task = Task { [scripted, interval] in
                for fix in scripted {
                    try? await Task.sleep(for: interval)
                    continuation.yield(fix)
                }
                continuation.finish()
            }
            continuation.onTermination = { _ in task.cancel() }
        }
    }
}

#if canImport(CoreLocation)
/// The real `CoreLocation`-backed provider. Bridges `CLLocationManager`'s
/// location + heading delegates into a single ``LocationFix`` stream.
public final class CoreLocationProvider: NSObject, LocationProvider, CLLocationManagerDelegate, @unchecked Sendable {
    private let manager = CLLocationManager()
    private var continuations: [UUID: AsyncStream<LocationFix>.Continuation] = [:]
    private var lastPosition: LatLng?
    private var lastHeading: Double?
    private let lock = NSLock()

    public override init() {
        super.init()
        manager.delegate = self
        manager.desiredAccuracy = kCLLocationAccuracyBest
    }

    public func requestAuthorization() {
        manager.requestWhenInUseAuthorization()
    }

    public func requestAlwaysAuthorization() {
        #if os(iOS)
        manager.requestAlwaysAuthorization()
        #else
        manager.requestWhenInUseAuthorization()
        #endif
    }

    public func setBackgroundUpdates(_ enabled: Bool) {
        #if os(iOS)
        // Requires the `location` UIBackgroundMode (set in Info.plist) — without
        // it this property assignment traps at runtime.
        manager.allowsBackgroundLocationUpdates = enabled
        manager.pausesLocationUpdatesAutomatically = false
        manager.showsBackgroundLocationIndicator = enabled
        #endif
    }

    public func fixes() -> AsyncStream<LocationFix> {
        AsyncStream { continuation in
            let key = UUID()
            lock.withLock { continuations[key] = continuation }
            manager.startUpdatingLocation()
            #if os(iOS)
            manager.startUpdatingHeading()
            #endif
            continuation.onTermination = { [weak self] _ in
                self?.lock.withLock { self?.continuations[key] = nil }
            }
        }
    }

    private var lastAltitude: Double?
    private var lastSpeed: Double?

    public func locationManager(_ manager: CLLocationManager, didUpdateLocations locations: [CLLocation]) {
        guard let loc = locations.last else { return }
        let position = LatLng(lat: loc.coordinate.latitude, lng: loc.coordinate.longitude)
        lock.withLock {
            lastPosition = position
            lastAltitude = loc.altitude
            lastSpeed = loc.speed >= 0 ? loc.speed : nil   // CL reports -1 when invalid
        }
        emit()
    }

    #if os(iOS)
    public func locationManager(_ manager: CLLocationManager, didUpdateHeading newHeading: CLHeading) {
        guard newHeading.headingAccuracy >= 0 else { return }
        lock.withLock { lastHeading = newHeading.trueHeading }
        emit()
    }
    #endif

    private func emit() {
        lock.withLock {
            guard let position = lastPosition else { return }
            let fix = LocationFix(position: position, headingDegrees: lastHeading, altitude: lastAltitude, speedMps: lastSpeed)
            for continuation in continuations.values { continuation.yield(fix) }
        }
    }
}
#endif
