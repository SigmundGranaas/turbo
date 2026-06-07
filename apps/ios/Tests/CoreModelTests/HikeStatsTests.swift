import Testing
import CoreModel

@Suite("HikeStats")
struct HikeStatsTests {

    private func hike() -> SavedPath {
        // ~ a short climb: 3 points, +14 then -6 elevation, 1800s moving.
        SavedPath(
            id: "h1", name: "Storheia",
            path: GeoPath(
                points: [LatLng(lat: 69.60, lng: 19.90), LatLng(lat: 69.61, lng: 19.92), LatLng(lat: 69.62, lng: 19.95)],
                source: .recording,
                elevations: [10, 24, 18],
                distanceM: 6000,
                movingTimeSeconds: 1800
            ),
            activityKind: .hiking
        )
    }

    @Test("derives distance, duration, ascent and max elevation")
    func basics() {
        let s = HikeStats(hike().path)
        #expect(s.distanceMeters == 6000)
        #expect(s.durationSeconds == 1800)
        #expect(s.ascentMeters == 14)        // +14 only (the -6 is descent)
        #expect(s.maxElevationMeters == 24)
    }

    @Test("average pace is duration per distance (min/km)")
    func pace() {
        let s = HikeStats(hike().path)
        // 1800s over 6 km = 300 s/km = 5:00 min/km
        #expect(s.averagePaceSecondsPerKm == 300)
        #expect(s.formattedPace == "5:00 /km")
    }

    @Test("formats distance and duration for display")
    func formatting() {
        let s = HikeStats(hike().path)
        #expect(s.formattedDistance == "6.0 km")
        #expect(s.formattedDuration == "30:00")     // mm:ss for < 1h
    }

    @Test("missing duration → no pace, nil-safe")
    func noDuration() {
        let path = GeoPath(points: [LatLng(lat: 0, lng: 0)], source: .saved, elevations: nil, distanceM: 1000)
        let s = HikeStats(path)
        #expect(s.durationSeconds == nil)
        #expect(s.averagePaceSecondsPerKm == nil)
        #expect(s.ascentMeters == nil)
    }

    @Test("elevation profile is the captured samples, empty when none")
    func profile() {
        #expect(HikeStats(hike().path).elevationProfile == [10, 24, 18])
        let flat = GeoPath(points: [], source: .saved)
        #expect(HikeStats(flat).elevationProfile.isEmpty)
    }
}
