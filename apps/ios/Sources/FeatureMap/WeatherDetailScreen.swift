import SwiftUI
import CoreModel
import CoreDesignSystem

/// Apple Weather-style forecast for a place — current conditions, hourly strip,
/// 10-day list, and a link to avalanche danger. Mirrors the WeatherKit design.
public struct WeatherDetailScreen: View {
    @Environment(\.turbo) private var t
    @Environment(\.dismiss) private var dismiss
    @State private var weather: WeatherViewModel
    private let avalancheViewModel: AvalancheViewModel

    public init(weather: WeatherViewModel, avalanche: AvalancheViewModel) {
        _weather = State(initialValue: weather)
        self.avalancheViewModel = avalanche
    }

    public var body: some View {
        NavigationStack {
            ScrollView {
                AsyncContent(
                    weather.state,
                    emptyTitle: "Weather Unavailable",
                    emptySymbol: "cloud.slash"
                ) { s in
                    VStack(spacing: 20) {
                        VStack(spacing: 6) {
                            Text(s.placeName).font(.turboTitle2).foregroundStyle(t.label)
                            Text(WeatherSummary.formatTemperature(s.temperatureC))
                                .font(.system(size: 72, weight: .thin)).foregroundStyle(t.label)
                            Label(s.summary, systemImage: s.symbol.sfSymbol)
                                .font(.turboSubhead).foregroundStyle(t.label2)
                                .multilineTextAlignment(.center)
                        }
                        .padding(.top, 12)

                        hourlyStrip(s.hourly)
                        if let sun = weather.sun { sunCard(sun) }
                        if let marine = weather.marine { marineCard(marine) }
                        dailyList(s.daily)

                        NavigationLink {
                            AvalancheDetailScreen(viewModel: avalancheViewModel)
                        } label: {
                            HStack(spacing: 12) {
                                Glyph(symbol: "exclamationmark.triangle.fill", color: t.orange, size: 29, cornerRadius: 7)
                                Text("Avalanche Danger").foregroundStyle(t.label)
                                Spacer()
                                Image(systemName: "chevron.right").font(.system(size: 14, weight: .semibold)).foregroundStyle(t.label3)
                            }
                            .padding(14)
                            .background(t.groupedCard, in: RoundedRectangle(cornerRadius: Radius.card, style: .continuous))
                        }
                        .accessibilityIdentifier("weather.avalanche")
                    }
                    .padding(16)
                }
            }
            .background(t.grouped)
            .navigationTitle("Weather")
            .toolbarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .confirmationAction) { Button("Done") { dismiss() } } }
            .task { await weather.load() }
        }
    }

    private func marineCard(_ marine: MarineConditions) -> some View {
        let cells: [(String, String, String)] = [
            marine.seaTemperatureC.map { ("thermometer.medium", "Sea", "\(Int($0.rounded()))°") },
            marine.waveHeightM.map { ("water.waves", "Waves", String(format: "%.1f m", $0)) },
            marine.seaCurrentMs.map { ("arrow.right.to.line", "Current", String(format: "%.1f m/s", $0)) },
        ].compactMap { $0 }
        return HStack(spacing: 0) {
            ForEach(Array(cells.enumerated()), id: \.offset) { idx, cell in
                if idx > 0 { Rectangle().fill(t.separator).frame(width: 0.5).padding(.vertical, 8) }
                sunCell(symbol: cell.0, title: cell.1, value: cell.2)
            }
        }
        .background(t.groupedCard, in: RoundedRectangle(cornerRadius: Radius.card, style: .continuous))
        .accessibilityIdentifier("weather.marine")
    }

    private func sunCard(_ sun: SunTimes) -> some View {
        HStack(spacing: 0) {
            sunCell(symbol: "sunrise.fill", title: "Sunrise", value: sunValue(sun.sunrise, polar: sun.polarNight ? "No sunrise" : nil))
            Rectangle().fill(t.separator).frame(width: 0.5).padding(.vertical, 8)
            sunCell(symbol: "sunset.fill", title: "Sunset", value: sunValue(sun.sunset, polar: sun.polarDay ? "No sunset" : nil))
        }
        .background(t.groupedCard, in: RoundedRectangle(cornerRadius: Radius.card, style: .continuous))
        .accessibilityIdentifier("weather.sun")
    }

    private func sunCell(symbol: String, title: String, value: String) -> some View {
        VStack(spacing: 6) {
            Label(title, systemImage: symbol).font(.turboFootnote).foregroundStyle(t.label2)
            Text(value).font(.turboTitle3).foregroundStyle(t.label)
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, 14)
    }

    private func sunValue(_ date: Date?, polar: String?) -> String {
        if let polar { return polar }
        guard let date else { return "—" }
        let f = DateFormatter()
        f.timeStyle = .short
        f.dateStyle = .none
        return f.string(from: date)
    }

    private func hourlyStrip(_ hours: [HourForecast]) -> some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 18) {
                ForEach(hours, id: \.label) { h in
                    VStack(spacing: 8) {
                        Text(h.label).font(.turboFootnote).foregroundStyle(t.label2)
                        Image(systemName: h.symbol.sfSymbol).foregroundStyle(t.blue)
                        Text(WeatherSummary.formatTemperature(h.temperatureC)).font(.turboHeadline).foregroundStyle(t.label)
                    }
                }
            }
            .padding(14)
        }
        .background(t.groupedCard, in: RoundedRectangle(cornerRadius: Radius.card, style: .continuous))
    }

    private func dailyList(_ days: [DayForecast]) -> some View {
        VStack(spacing: 0) {
            ForEach(days) { day in
                HStack {
                    Text(day.weekday).font(.turboBody).foregroundStyle(t.label).frame(width: 64, alignment: .leading)
                    Image(systemName: day.symbol.sfSymbol).foregroundStyle(t.blue)
                    Spacer()
                    Text(WeatherSummary.formatTemperature(day.lowC)).foregroundStyle(t.label2)
                    Text(WeatherSummary.formatTemperature(day.highC)).foregroundStyle(t.label)
                }
                .padding(.horizontal, 14).padding(.vertical, 10)
                if day.id != days.last?.id {
                    Rectangle().fill(t.separator).frame(height: 0.5).padding(.leading, 14)
                }
            }
        }
        .background(t.groupedCard, in: RoundedRectangle(cornerRadius: Radius.card, style: .continuous))
    }
}
