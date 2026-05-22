using FluentAssertions;
using NetTopologySuite.Geometries;
using Turboapi.Activities.BackcountrySki.conditions;
using Turboapi.Activities.BackcountrySki.domain;
using Turboapi.Activities.BackcountrySki.value;
using Turboapi.Activities.domain;
using Turboapi.Activities.value;
using Xunit;

namespace Turbo.Activities.Unit;

public sealed class BackcountrySkiConditionsAdvisorTests
{
    private static readonly GeometryFactory F = new(new PrecisionModel(), 4326);

    private sealed class StubWeather : IWeatherProvider
    {
        private readonly WeatherSlice _slice;
        public StubWeather(WeatherSlice slice) => _slice = slice;
        public string Key => "stub";
        public Task<WeatherSlice> GetAsync(double latitude, double longitude, DateTimeOffset at, CancellationToken cancellationToken)
            => Task.FromResult(_slice);
    }

    private static BackcountrySkiActivity MakeActivity(
        AtesRating ates = AtesRating.Challenging,
        short? preferredMax = null)
    {
        var details = new BackcountrySkiDetails(
            ascentMeters: 800, descentMeters: 800, distanceMeters: 6500,
            elevationMinMeters: 900, elevationMaxMeters: 1700,
            atesRating: ates, dominantAspect: Aspect.N,
            varsomRegionId: 3014, preferredAvalancheMaxLevel: preferredMax,
            aspectMix: null, legs: null);
        var route = F.CreateLineString(new[]
        {
            new Coordinate(7.0, 61.0), new Coordinate(7.01, 61.005), new Coordinate(7.02, 61.01),
        });
        var core = ActivityCore.New(Guid.NewGuid(), "Route", null, route);
        return BackcountrySkiActivity.Create(core, details);
    }

    private static WeatherSlice Stable() => new(
        validAt: DateTimeOffset.UtcNow,
        airTemperatureCelsius: -5, airPressureHpa: 1015, relativeHumidityPct: 70, cloudCoveragePct: 30,
        windSpeedMs: 5, windGustMs: 7, windFromDegrees: 90,
        precipitationNext1hMm: null, precipitationNext6hMm: null, symbolCode: "partlycloudy_day");

    private static WeatherSlice Windloading() => new(
        validAt: DateTimeOffset.UtcNow,
        airTemperatureCelsius: -8, airPressureHpa: 998, relativeHumidityPct: 90, cloudCoveragePct: 90,
        windSpeedMs: 20, windGustMs: 30, windFromDegrees: 270,
        precipitationNext1hMm: null, precipitationNext6hMm: 25, symbolCode: "snow");

    private static WeatherSlice ThawDay() => new(
        validAt: DateTimeOffset.UtcNow,
        airTemperatureCelsius: 6, airPressureHpa: 1010, relativeHumidityPct: 80, cloudCoveragePct: 60,
        windSpeedMs: 4, windGustMs: 6, windFromDegrees: 180,
        precipitationNext1hMm: null, precipitationNext6hMm: null, symbolCode: "partlycloudy_day");

    [Fact]
    public async Task Stable_weather_gives_high_or_null_score_with_varsom_reminder()
    {
        var advisor = new BackcountrySkiConditionsAdvisor(new StubWeather(Stable()));
        var report = await advisor.AdviseAsync(MakeActivity(), DateTimeOffset.UtcNow, CancellationToken.None);
        report.Rationale.Should().Contain("Varsom");
        report.AvalancheLevel.Should().BeNull(); // placeholder until provider lands
    }

    [Fact]
    public async Task Strong_wind_plus_fresh_snow_drives_score_down_and_lists_wind_loading()
    {
        var advisor = new BackcountrySkiConditionsAdvisor(new StubWeather(Windloading()));
        var report = await advisor.AdviseAsync(MakeActivity(), DateTimeOffset.UtcNow, CancellationToken.None);
        report.Score.Should().NotBeNull().And.BeLessThan(50);
        report.Rationale.Should().Contain("wind loading");
    }

    [Fact]
    public async Task Above_freezing_temperature_in_winter_route_penalizes_score()
    {
        var advisor = new BackcountrySkiConditionsAdvisor(new StubWeather(ThawDay()));
        var report = await advisor.AdviseAsync(MakeActivity(), DateTimeOffset.UtcNow, CancellationToken.None);
        report.Rationale.Should().Contain("wet snow");
    }

    [Fact]
    public async Task Complex_terrain_with_low_preferred_max_surfaces_varsom_data_gap_note()
    {
        // No IAvalancheProvider passed → the advisor falls back to the
        // "verify Varsom before going" rationale.
        var advisor = new BackcountrySkiConditionsAdvisor(new StubWeather(Stable()), avalanche: null);
        var report = await advisor.AdviseAsync(
            MakeActivity(ates: AtesRating.Complex, preferredMax: 2),
            DateTimeOffset.UtcNow, CancellationToken.None);
        report.Rationale.Should().Contain("avalanche data unavailable");
    }

    private sealed class StubAvalanche : IAvalancheProvider
    {
        private readonly AvalancheSlice _slice;
        public StubAvalanche(AvalancheSlice slice) => _slice = slice;
        public string Key => "stub_avalanche";
        public Task<AvalancheSlice> GetAsync(int varsomRegionId, DateTimeOffset at, CancellationToken cancellationToken)
            => Task.FromResult(_slice);
    }

    [Fact]
    public async Task Level_3_with_complex_ATES_penalises_score()
    {
        var avalanche = new AvalancheSlice(DateTimeOffset.UtcNow, dangerLevel: 3,
            summary: "Considerable", problems: "WindSlab");
        var advisor = new BackcountrySkiConditionsAdvisor(
            new StubWeather(Stable()), new StubAvalanche(avalanche));
        var report = await advisor.AdviseAsync(
            MakeActivity(ates: AtesRating.Complex),
            DateTimeOffset.UtcNow, CancellationToken.None);
        report.AvalancheLevel.Should().Be(3);
        report.Rationale.Should().Contain("complex ATES");
        report.Score.Should().NotBeNull().And.BeLessThan(80);
    }

    [Fact]
    public async Task Level_5_kills_score_with_avoid_message()
    {
        var avalanche = new AvalancheSlice(DateTimeOffset.UtcNow, dangerLevel: 5,
            summary: "Extreme", problems: "WindSlab,WetSnow");
        var advisor = new BackcountrySkiConditionsAdvisor(
            new StubWeather(Stable()), new StubAvalanche(avalanche));
        var report = await advisor.AdviseAsync(
            MakeActivity(), DateTimeOffset.UtcNow, CancellationToken.None);
        report.Rationale.Should().Contain("avoid all avalanche terrain");
        report.Score.Should().NotBeNull().And.BeLessThan(25);
    }

    [Fact]
    public async Task Forecast_above_user_preferred_max_appears_in_rationale()
    {
        var avalanche = new AvalancheSlice(DateTimeOffset.UtcNow, dangerLevel: 3,
            summary: "Considerable", problems: "");
        var advisor = new BackcountrySkiConditionsAdvisor(
            new StubWeather(Stable()), new StubAvalanche(avalanche));
        var report = await advisor.AdviseAsync(
            MakeActivity(preferredMax: 2), DateTimeOffset.UtcNow, CancellationToken.None);
        report.Rationale.Should().Contain("exceeds your max");
    }
}
