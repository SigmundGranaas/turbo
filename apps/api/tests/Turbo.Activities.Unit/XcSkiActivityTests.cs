using FluentAssertions;
using NetTopologySuite.Geometries;
using Turboapi.Activities.domain;
using Turboapi.Activities.XcSki.domain;
using Turboapi.Activities.XcSki.value;
using Xunit;

namespace Turbo.Activities.Unit;

public sealed class XcSkiActivityTests
{
    private static readonly GeometryFactory F = new(new PrecisionModel(), 4326);

    private static LineString Route() => F.CreateLineString(new[]
    {
        new Coordinate(11.0, 60.5), new Coordinate(11.005, 60.51),
    });

    private static XcSkiDetails Details(int distanceMeters = 5000) => new(
        distanceMeters, ascentMeters: 50, descentMeters: 50,
        technique: XcSkiTechnique.Both,
        groomingStatus: GroomingStatus.Today,
        isLit: true, requiresSeasonPass: false,
        groomingFeedKey: null);

    [Fact]
    public void Create_with_LineString_succeeds()
    {
        var core = ActivityCore.New(Guid.NewGuid(), "Loypa", null, Route());
        var x = XcSkiActivity.Create(core, Details());
        x.Details.Technique.Should().Be(XcSkiTechnique.Both);
        x.Details.IsLit.Should().BeTrue();
    }

    [Fact]
    public void Create_rejects_point_geometry()
    {
        var core = ActivityCore.New(Guid.NewGuid(), "n", null, F.CreatePoint(new Coordinate(11, 60.5)));
        var act = () => XcSkiActivity.Create(core, Details());
        act.Should().Throw<ArgumentException>().WithMessage("*trail-based*");
    }

    [Fact]
    public void Create_rejects_negative_distance()
    {
        var core = ActivityCore.New(Guid.NewGuid(), "n", null, Route());
        var act = () => XcSkiActivity.Create(core, Details(distanceMeters: -5));
        act.Should().Throw<ArgumentException>().WithMessage("*non-negative*");
    }
}
