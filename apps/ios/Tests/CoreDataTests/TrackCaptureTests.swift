import Testing
import CoreModel
import CoreData

@Suite("TrackCapture")
struct TrackCaptureTests {

    private func fold(_ fixes: [LocationFix]) -> CapturedTrack {
        fixes.reduce(CapturedTrack()) { TrackCapture.append($0, $1) }
    }

    @Test("appendDetached adds the point but not the connecting gap distance")
    func detached() {
        let base = fold([
            LocationFix(position: LatLng(lat: 0, lng: 0.000), altitude: 100),
            LocationFix(position: LatLng(lat: 0, lng: 0.003), altitude: 110),
        ])
        let d0 = base.distanceM
        // A far jump appended detached should NOT grow the distance (the gap isn't counted)…
        let jumped = TrackCapture.appendDetached(base, LocationFix(position: LatLng(lat: 0, lng: 1.0), altitude: 120))
        #expect(jumped.points.count == 3)
        #expect(jumped.distanceM == d0)
        // …but a normal append from the new point counts again.
        let next = TrackCapture.append(jumped, LocationFix(position: LatLng(lat: 0, lng: 1.003), altitude: 130))
        #expect(next.distanceM > d0)
    }

    @Test("accumulates points, distance, ascent/descent and peak speed")
    func accumulates() {
        let track = fold([
            LocationFix(position: LatLng(lat: 0, lng: 0.000), altitude: 100, speedMps: 1.0),
            LocationFix(position: LatLng(lat: 0, lng: 0.003), altitude: 130, speedMps: 2.5),
            LocationFix(position: LatLng(lat: 0, lng: 0.006), altitude: 120, speedMps: 2.0),
        ])
        #expect(track.points.count == 3)
        #expect(track.elevations == [100, 130, 120])
        #expect(track.distanceM > 600)
        #expect(track.ascentM == 30)   // 100→130
        #expect(track.descentM == 10)  // 130→120
        #expect(track.currentSpeedMps == 2.0) // latest
        #expect(track.maxSpeedMps == 2.5)     // peak
    }

    @Test("a fix without altitude does not grow the elevation series")
    func altitudeOptional() {
        let track = fold([
            LocationFix(position: LatLng(lat: 0, lng: 0.000), altitude: 100),
            LocationFix(position: LatLng(lat: 0, lng: 0.003), altitude: nil),
        ])
        #expect(track.points.count == 2)
        #expect(track.elevations == [100])
        #expect(track.currentAltitude == 100)
    }

    @Test("identical fixes fold to an identical track regardless of caller (record vs follow)")
    func parity() {
        let fixes = (0...8).map { LocationFix(position: LatLng(lat: 0, lng: Double($0) * 0.003), altitude: 100 + Double($0)) }
        let viaRecord = fold(fixes)
        let viaFollow = fold(fixes)
        #expect(viaRecord == viaFollow)
    }
}
