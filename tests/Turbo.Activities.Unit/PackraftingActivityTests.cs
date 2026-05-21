using FluentAssertions;
using NetTopologySuite.Geometries;
using Turboapi.Activities.domain;
using Turboapi.Activities.Packrafting.domain;
using Turboapi.Activities.Packrafting.value;
using Xunit;

namespace Turbo.Activities.Unit;

public sealed class PackraftingActivityTests
{
    private static readonly GeometryFactory F = new(new PrecisionModel(), 4326);

    private static LineString Route() => F.CreateLineString(new[]
    {
        new Coordinate(7.0, 61.0), new Coordinate(7.05, 61.02),
    });

    private static PackraftingDetails Details(
        WaterGrade typicalGrade = WaterGrade.II,
        WaterGrade maxGrade = WaterGrade.III) => new(
        distanceMeters: 12000,
        paddleDistanceMeters: 10000,
        portageDistanceMeters: 2000,
        maxGrade: maxGrade,
        typicalGrade: typicalGrade,
        putInLat: 61.0, putInLon: 7.0,
        takeOutLat: 61.02, takeOutLon: 7.05,
        nveStationCode: null,
        minFlowCumecs: null,
        maxFlowCumecs: null,
        segments: null);

    [Fact]
    public void Create_with_LineString_succeeds()
    {
        var core = ActivityCore.New(Guid.NewGuid(), "Elva", null, Route());
        var p = PackraftingActivity.Create(core, Details());
        p.Details.MaxGrade.Should().Be(WaterGrade.III);
    }

    [Fact]
    public void Create_rejects_typicalGrade_above_maxGrade()
    {
        var core = ActivityCore.New(Guid.NewGuid(), "n", null, Route());
        var act = () => PackraftingActivity.Create(core, Details(typicalGrade: WaterGrade.IV, maxGrade: WaterGrade.II));
        act.Should().Throw<ArgumentException>().WithMessage("*typicalGrade*maxGrade*");
    }
}
