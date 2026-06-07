import Testing
import CoreModel
@testable import CoreData

@Suite("Conditions providers")
struct ConditionsProvidersTests {

    @Test("weather provider returns a populated forecast")
    func weather() async {
        let summary = await InMemoryWeatherProvider().forecast(at: LatLng(lat: 69.6, lng: 20.0), placeName: "Lyngen")
        #expect(summary.placeName == "Lyngen")
        #expect(!summary.hourly.isEmpty)
        #expect(summary.daily.count >= 7)
    }

    @Test("avalanche provider returns a level in 1...5")
    func avalanche() async {
        let info = await InMemoryAvalancheProvider().danger(at: LatLng(lat: 69.6, lng: 20.0))
        #expect((1...5).contains(info.level))
        #expect(!info.headline.isEmpty)
    }

    @Test("temperature formats with a real minus sign")
    func tempFormat() {
        #expect(WeatherSummary.formatTemperature(-3) == "−3°")
        #expect(WeatherSummary.formatTemperature(5) == "5°")
    }

    @Test("avalanche labels map the European scale")
    func labels() {
        #expect(AvalancheInfo(region: "x", level: 1, headline: "h").label == "Low")
        #expect(AvalancheInfo(region: "x", level: 4, headline: "h").label == "High")
        #expect(AvalancheInfo(region: "x", level: 9, headline: "h").level == 5)  // clamped
    }
}
