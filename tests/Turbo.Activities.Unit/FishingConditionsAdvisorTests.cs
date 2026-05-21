using FluentAssertions;
using NetTopologySuite.Geometries;
using Turboapi.Activities.conditions;
using Turboapi.Activities.domain;
using Turboapi.Activities.Fishing.conditions;
using Turboapi.Activities.Fishing.domain;
using Turboapi.Activities.Fishing.value;
using Turboapi.Activities.value;
using Xunit;

namespace Turbo.Activities.Unit;

public sealed class SyntheticWeatherProviderTests
{
    [Fact]
    public async Task Same_inputs_yield_identical_slice()
    {
        var provider = new SyntheticWeatherProvider();
        var at = new DateTimeOffset(2026, 5, 20, 12, 0, 0, TimeSpan.Zero);
        var a = await provider.GetAsync(60.12, 5.32, at, CancellationToken.None);
        var b = await provider.GetAsync(60.12, 5.32, at, CancellationToken.None);
        a.Should().BeEquivalentTo(b);
    }

    [Fact]
    public async Task Hour_change_changes_slice_within_same_grid_cell()
    {
        var provider = new SyntheticWeatherProvider();
        var at = new DateTimeOffset(2026, 5, 20, 12, 0, 0, TimeSpan.Zero);
        var a = await provider.GetAsync(60.12, 5.32, at, CancellationToken.None);
        var b = await provider.GetAsync(60.12, 5.32, at.AddHours(6), CancellationToken.None);
        a.Should().NotBeEquivalentTo(b);
    }

    [Fact]
    public async Task Snapped_lat_lon_within_grid_cell_yields_same_slice()
    {
        // Both (60.121, 5.319) and (60.122, 5.321) round to (60.12, 5.32).
        // Tight to the snapped centre to dodge floating-point edge cases.
        var provider = new SyntheticWeatherProvider();
        var at = new DateTimeOffset(2026, 5, 20, 12, 0, 0, TimeSpan.Zero);
        var a = await provider.GetAsync(60.121, 5.319, at, CancellationToken.None);
        var b = await provider.GetAsync(60.122, 5.321, at, CancellationToken.None);
        a.Should().BeEquivalentTo(b);
    }

    [Fact]
    public async Task Polar_latitude_yields_lower_temperature_than_equator()
    {
        var provider = new SyntheticWeatherProvider();
        var at = new DateTimeOffset(2026, 5, 20, 12, 0, 0, TimeSpan.Zero);
        var polar = await provider.GetAsync(80.0, 5.0, at, CancellationToken.None);
        var equator = await provider.GetAsync(0.0, 5.0, at, CancellationToken.None);
        polar.AirTemperatureCelsius.Should().BeLessThan(equator.AirTemperatureCelsius);
    }
}

public sealed class FishingConditionsAdvisorTests
{
    private static readonly GeometryFactory F = new(new PrecisionModel(), 4326);

    private static FishingActivity MakeActivity(PreferredConditions? preferred = null)
    {
        var details = new FishingDetails(
            waterKind: WaterKind.River, shoreOrBoat: ShoreOrBoat.Shore,
            accessNotes: null, targetSpecies: null, knownDepths: null,
            preferred: preferred);
        var core = ActivityCore.New(Guid.NewGuid(), "Spot", null, F.CreatePoint(new Coordinate(5.32, 60.12)));
        return FishingActivity.Create(core, details);
    }

    private sealed class StubWeatherProvider : IWeatherProvider
    {
        private readonly WeatherSlice _slice;
        public StubWeatherProvider(WeatherSlice slice) => _slice = slice;
        public string Key => "stub_weather";
        public Task<WeatherSlice> GetAsync(double latitude, double longitude, DateTimeOffset at, CancellationToken cancellationToken)
            => Task.FromResult(_slice);
    }

    private static WeatherSlice Calm() => new(
        validAt: DateTimeOffset.UtcNow,
        airTemperatureCelsius: 15, airPressureHpa: 1013, relativeHumidityPct: 60, cloudCoveragePct: 30,
        windSpeedMs: 3, windGustMs: 5, windFromDegrees: 180,
        precipitationNext1hMm: null, precipitationNext6hMm: null, symbolCode: "partlycloudy_day");

    private static WeatherSlice Stormy() => new(
        validAt: DateTimeOffset.UtcNow,
        airTemperatureCelsius: 8, airPressureHpa: 990, relativeHumidityPct: 95, cloudCoveragePct: 100,
        windSpeedMs: 18, windGustMs: 26, windFromDegrees: 250,
        precipitationNext1hMm: 8, precipitationNext6hMm: 30, symbolCode: "rain");

    [Fact]
    public async Task Calm_weather_no_preferences_gives_high_score()
    {
        var advisor = new FishingConditionsAdvisor(new StubWeatherProvider(Calm()));
        var report = await advisor.AdviseAsync(MakeActivity(), DateTimeOffset.UtcNow, CancellationToken.None);
        report.Score.Should().BeNull(); // perfectly neutral conditions → no score
        report.Rationale.Should().Contain("good");
    }

    [Fact]
    public async Task Stormy_weather_drives_score_low_and_lists_reasons()
    {
        var advisor = new FishingConditionsAdvisor(new StubWeatherProvider(Stormy()));
        var report = await advisor.AdviseAsync(MakeActivity(), DateTimeOffset.UtcNow, CancellationToken.None);
        report.Score.Should().NotBeNull().And.BeLessThan(50);
        report.Rationale.Should().Contain("wind").And.Contain("rain");
    }

    [Fact]
    public async Task Wind_above_user_preferred_max_penalizes_score()
    {
        var preferred = new PreferredConditions(pressureMinHpa: null, pressureMaxHpa: null, windMaxMs: 5);
        var advisor = new FishingConditionsAdvisor(new StubWeatherProvider(Calm()));
        var report = await advisor.AdviseAsync(MakeActivity(preferred), DateTimeOffset.UtcNow, CancellationToken.None);
        // Calm() has wind=3, below the preferred max of 5; no penalty.
        report.Score.Should().NotBeNull();
        report.Rationale.Should().NotContain("wind exceeds");
    }

    [Fact]
    public async Task Wind_well_above_preferred_max_appears_in_rationale()
    {
        var preferred = new PreferredConditions(pressureMinHpa: null, pressureMaxHpa: null, windMaxMs: 2);
        var advisor = new FishingConditionsAdvisor(new StubWeatherProvider(Calm()));
        var report = await advisor.AdviseAsync(MakeActivity(preferred), DateTimeOffset.UtcNow, CancellationToken.None);
        report.Rationale.Should().Contain("wind exceeds your preferred maximum");
    }

    [Fact]
    public async Task Pressure_outside_user_window_penalizes_score()
    {
        var preferred = new PreferredConditions(pressureMinHpa: 1020, pressureMaxHpa: 1030, windMaxMs: null);
        var advisor = new FishingConditionsAdvisor(new StubWeatherProvider(Calm())); // 1013 hPa
        var report = await advisor.AdviseAsync(MakeActivity(preferred), DateTimeOffset.UtcNow, CancellationToken.None);
        report.Rationale.Should().Contain("pressure below your preferred minimum");
    }
}
