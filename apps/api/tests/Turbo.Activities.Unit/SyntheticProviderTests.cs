using FluentAssertions;
using Turboapi.Activities.conditions;
using Xunit;

namespace Turbo.Activities.Unit;

/// <summary>
/// Determinism tests for the synthetic providers that back conditions
/// when no upstream is wired in. Each provider must be a pure function
/// of its inputs so repeat lookups, caching, and reload-tolerance all
/// behave predictably. SyntheticWeatherProvider has its own dedicated
/// suite (see FishingConditionsAdvisorTests) so we cover the other four
/// here.
/// </summary>
public sealed class SyntheticRiverFlowProviderTests
{
    [Fact]
    public async Task Same_inputs_yield_identical_slice()
    {
        var p = new SyntheticRiverFlowProvider();
        var at = new DateTimeOffset(2026, 5, 20, 12, 0, 0, TimeSpan.Zero);
        var a = await p.GetAsync("NVE-12.34.0", at, CancellationToken.None);
        var b = await p.GetAsync("NVE-12.34.0", at, CancellationToken.None);
        a.Should().BeEquivalentTo(b);
    }

    [Fact]
    public async Task Different_stations_yield_different_baseline_flow()
    {
        var p = new SyntheticRiverFlowProvider();
        var at = new DateTimeOffset(2026, 5, 20, 12, 0, 0, TimeSpan.Zero);
        var a = await p.GetAsync("NVE-12.34.0", at, CancellationToken.None);
        var b = await p.GetAsync("NVE-99.88.7", at, CancellationToken.None);
        a.CurrentCumecs.Should().NotBe(b.CurrentCumecs);
    }

    [Fact]
    public async Task Spring_melt_window_has_higher_flow_than_winter()
    {
        var p = new SyntheticRiverFlowProvider();
        var spring = new DateTimeOffset(2026, 6, 1, 12, 0, 0, TimeSpan.Zero);
        var winter = new DateTimeOffset(2026, 1, 15, 12, 0, 0, TimeSpan.Zero);
        var s = await p.GetAsync("NVE-12.34.0", spring, CancellationToken.None);
        var w = await p.GetAsync("NVE-12.34.0", winter, CancellationToken.None);
        s.CurrentCumecs.Should().BeGreaterThan(w.CurrentCumecs);
    }

    [Fact]
    public async Task Trend_string_is_one_of_three_values()
    {
        var p = new SyntheticRiverFlowProvider();
        var at = new DateTimeOffset(2026, 5, 20, 12, 0, 0, TimeSpan.Zero);
        var s = await p.GetAsync("NVE-12.34.0", at, CancellationToken.None);
        s.Trend.Should().BeOneOf("rising", "falling", "stable");
    }
}

public sealed class SyntheticTideProviderTests
{
    [Fact]
    public async Task Same_inputs_yield_identical_slice()
    {
        var p = new SyntheticTideProvider();
        var at = new DateTimeOffset(2026, 5, 20, 12, 0, 0, TimeSpan.Zero);
        var a = await p.GetAsync(60.12, 5.32, at, CancellationToken.None);
        var b = await p.GetAsync(60.12, 5.32, at, CancellationToken.None);
        a.Should().BeEquivalentTo(b);
    }

    [Fact]
    public async Task Same_grid_cell_share_phase_summary()
    {
        // Tide phase snaps at 0.01° (×100 round). 60.121 and 60.122 both → 60.12.
        // The amplitude is a smooth function of latitude (not the snapped value),
        // so heights differ very slightly across mates in the same cell — but the
        // rising/falling/slack summary is phase-driven and stays consistent.
        var p = new SyntheticTideProvider();
        var at = new DateTimeOffset(2026, 5, 20, 12, 0, 0, TimeSpan.Zero);
        var a = await p.GetAsync(60.121, 5.319, at, CancellationToken.None);
        var b = await p.GetAsync(60.122, 5.321, at, CancellationToken.None);
        a.Summary.Should().Be(b.Summary);
        a.CurrentHeightMeters.Should().BeApproximately(b.CurrentHeightMeters, 0.05f);
    }

    [Fact]
    public async Task Summary_uses_tide_vocabulary()
    {
        var p = new SyntheticTideProvider();
        var at = new DateTimeOffset(2026, 5, 20, 12, 0, 0, TimeSpan.Zero);
        var s = await p.GetAsync(60.12, 5.32, at, CancellationToken.None);
        s.Summary.Should().BeOneOf("slack", "rising tide", "falling tide");
    }
}

public sealed class SyntheticAvalancheProviderTests
{
    [Fact]
    public async Task Same_region_and_day_yield_identical_slice()
    {
        var p = new SyntheticAvalancheProvider();
        var at = new DateTimeOffset(2026, 1, 15, 12, 0, 0, TimeSpan.Zero);
        var a = await p.GetAsync(3010, at, CancellationToken.None);
        var b = await p.GetAsync(3010, at, CancellationToken.None);
        a.Should().BeEquivalentTo(b);
    }

    [Fact]
    public async Task Time_of_day_does_not_change_daily_bulletin()
    {
        var p = new SyntheticAvalancheProvider();
        var morning = new DateTimeOffset(2026, 1, 15, 8, 0, 0, TimeSpan.Zero);
        var evening = new DateTimeOffset(2026, 1, 15, 20, 0, 0, TimeSpan.Zero);
        var a = await p.GetAsync(3010, morning, CancellationToken.None);
        var b = await p.GetAsync(3010, evening, CancellationToken.None);
        a.Should().BeEquivalentTo(b);
    }

    [Fact]
    public async Task Level_is_in_valid_range()
    {
        var p = new SyntheticAvalancheProvider();
        for (var day = 1; day <= 28; day++)
        {
            var at = new DateTimeOffset(2026, 1, day, 12, 0, 0, TimeSpan.Zero);
            var s = await p.GetAsync(3010, at, CancellationToken.None);
            s.DangerLevel.Should().BeInRange(1, 5);
        }
    }

    [Fact]
    public async Task Different_regions_can_diverge_on_same_day()
    {
        // Across many regions we expect at least one to differ from region 3010.
        var p = new SyntheticAvalancheProvider();
        var at = new DateTimeOffset(2026, 1, 15, 12, 0, 0, TimeSpan.Zero);
        var baseline = await p.GetAsync(3010, at, CancellationToken.None);
        var any = false;
        for (var r = 3011; r <= 3030; r++)
        {
            var s = await p.GetAsync(r, at, CancellationToken.None);
            if (s.DangerLevel != baseline.DangerLevel || s.Problems != baseline.Problems) { any = true; break; }
        }
        any.Should().BeTrue();
    }
}

public sealed class SyntheticGroomingProviderTests
{
    [Fact]
    public async Task Same_feed_and_day_yield_identical_slice()
    {
        var p = new SyntheticGroomingProvider();
        var at = new DateTimeOffset(2026, 1, 15, 12, 0, 0, TimeSpan.Zero);
        var a = await p.GetAsync("oslo/holmenkollen", at, CancellationToken.None);
        var b = await p.GetAsync("oslo/holmenkollen", at, CancellationToken.None);
        a.Should().BeEquivalentTo(b);
    }

    [Fact]
    public async Task Winter_groomings_are_recent_summer_are_stale()
    {
        var p = new SyntheticGroomingProvider();
        var winter = new DateTimeOffset(2026, 1, 15, 12, 0, 0, TimeSpan.Zero);
        var summer = new DateTimeOffset(2026, 7, 15, 12, 0, 0, TimeSpan.Zero);
        var w = await p.GetAsync("oslo/holmenkollen", winter, CancellationToken.None);
        var s = await p.GetAsync("oslo/holmenkollen", summer, CancellationToken.None);
        w.HoursAgo.Should().BeLessThan(s.HoursAgo);
    }
}
