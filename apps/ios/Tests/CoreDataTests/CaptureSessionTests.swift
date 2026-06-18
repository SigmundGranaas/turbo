import Testing
import CoreModel
@testable import CoreData

/// The shared pause-buffer capture engine behind both recording and following (US-4).
@Suite("CaptureSession")
struct CaptureSessionTests {
    private func fix(_ lat: Double, _ lng: Double = 18.0, alt: Double? = nil, speed: Double? = nil) -> LocationFix {
        LocationFix(position: LatLng(lat: lat, lng: lng), altitude: alt, speedMps: speed)
    }

    @Test("active fixes accumulate into the committed track")
    func active() {
        let s = CaptureSession().appending(fix(69.0)).appending(fix(69.001)) // ~111 m
        #expect(s.track.points.count == 2)
        #expect(!s.paused)
        #expect(s.track.distanceM > 100 && s.track.distanceM < 125)
        #expect(s.bufferedDistanceM == 0)
    }

    @Test("paused fixes buffer instead of touching the track")
    func pausedBuffers() {
        let s = CaptureSession().appending(fix(69.0)).paused().appending(fix(69.001))
        #expect(s.paused)
        #expect(s.track.points.count == 1)            // track frozen
        #expect(s.bufferedDistanceM > 90)             // the paused walk was captured
        #expect(s.hasBufferedMovement)
    }

    @Test("resume include stitches the paused walk onto the track")
    func resumeInclude() {
        let s = CaptureSession().appending(fix(69.0)).paused()
            .appending(fix(69.001)).appending(fix(69.002))
            .resuming(include: true)
        #expect(!s.paused)
        #expect(s.track.points.count == 3)
        #expect(s.bufferedDistanceM == 0)
        #expect(s.track.distanceM > 200)
    }

    @Test("resume discard drops the walk and lifts the pen so the gap is not counted")
    func resumeDiscard() {
        var s = CaptureSession().appending(fix(69.0)).paused().appending(fix(69.001)).resuming(include: false)
        #expect(s.track.points.count == 1)
        #expect(s.track.distanceM == 0)
        // First fix after discard is detached: no gap distance back to the old point.
        s = s.appending(fix(69.001))
        #expect(s.track.points.count == 2)
        #expect(s.track.distanceM == 0)
        // …then normal accumulation resumes.
        s = s.appending(fix(69.002))
        #expect(s.track.distanceM > 90)
    }
}
