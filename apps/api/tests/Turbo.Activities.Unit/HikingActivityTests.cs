using FluentAssertions;
using NetTopologySuite.Geometries;
using Turboapi.Activities.domain;
using Turboapi.Activities.Hiking.domain;
using Turboapi.Activities.Hiking.value;
using Xunit;

namespace Turbo.Activities.Unit;

public sealed class HikingActivityTests
{
    private static readonly GeometryFactory F = new(new PrecisionModel(), 4326);

    private static LineString Route() => F.CreateLineString(new[]
    {
        new Coordinate(10.0, 60.0), new Coordinate(10.01, 60.01), new Coordinate(10.02, 60.02),
    });

    private static HikingDetails Details(
        int distanceMeters = 8000,
        int elevationMinMeters = 600,
        int elevationMaxMeters = 1200) => new(
        distanceMeters, ascentMeters: 600, descentMeters: 600,
        elevationMinMeters, elevationMaxMeters,
        difficulty: HikingDifficulty.Moderate,
        surface: TrailSurface.Path,
        marking: TrailMarking.Signposted,
        estimatedHours: 4.5f,
        hasWaterSources: true, hasShelter: false,
        waterSources: null);

    [Fact]
    public void Create_with_LineString_succeeds()
    {
        var core = ActivityCore.New(Guid.NewGuid(), "Trail", null, Route());
        var h = HikingActivity.Create(core, Details());
        h.Details.Difficulty.Should().Be(HikingDifficulty.Moderate);
    }

    [Fact]
    public void Create_rejects_point_geometry()
    {
        var core = ActivityCore.New(Guid.NewGuid(), "n", null, F.CreatePoint(new Coordinate(10, 60)));
        var act = () => HikingActivity.Create(core, Details());
        act.Should().Throw<ArgumentException>().WithMessage("*route-based*");
    }

    [Fact]
    public void Create_rejects_max_below_min_elevation()
    {
        var core = ActivityCore.New(Guid.NewGuid(), "n", null, Route());
        var act = () => HikingActivity.Create(core, Details(elevationMinMeters: 1500, elevationMaxMeters: 1200));
        act.Should().Throw<ArgumentException>().WithMessage("*elevationMaxMeters*");
    }
}
