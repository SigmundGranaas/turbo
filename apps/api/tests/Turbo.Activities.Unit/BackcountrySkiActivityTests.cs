using FluentAssertions;
using NetTopologySuite.Geometries;
using Turboapi.Activities.BackcountrySki.domain;
using Turboapi.Activities.BackcountrySki.value;
using Turboapi.Activities.domain;
using Xunit;

namespace Turbo.Activities.Unit;

public sealed class BackcountrySkiActivityTests
{
    private static readonly GeometryFactory F = new(new PrecisionModel(), 4326);

    private static LineString Route() => F.CreateLineString(new[]
    {
        new Coordinate(8.0, 60.0),
        new Coordinate(8.01, 60.01),
        new Coordinate(8.02, 60.02),
    });

    private static BackcountrySkiDetails Details(
        int ascentMeters = 800,
        int descentMeters = 800,
        int distanceMeters = 6500,
        int elevationMinMeters = 900,
        int elevationMaxMeters = 1700,
        AtesRating atesRating = AtesRating.Challenging,
        Aspect? dominantAspect = Aspect.N,
        IReadOnlyList<AspectShare>? aspectMix = null,
        IReadOnlyList<RouteLeg>? legs = null) => new(
        ascentMeters, descentMeters, distanceMeters,
        elevationMinMeters, elevationMaxMeters,
        atesRating, dominantAspect,
        varsomRegionId: 3014,
        preferredAvalancheMaxLevel: 2,
        aspectMix: aspectMix,
        legs: legs);

    [Fact]
    public void Create_with_LineString_succeeds()
    {
        var core = ActivityCore.New(Guid.NewGuid(), "Galdhøpiggen N", null, Route());
        var bcski = BackcountrySkiActivity.Create(core, Details());
        bcski.Route.NumPoints.Should().Be(3);
        bcski.Details.AtesRating.Should().Be(AtesRating.Challenging);
    }

    [Fact]
    public void Create_rejects_point_geometry()
    {
        var core = ActivityCore.New(Guid.NewGuid(), "n", null, F.CreatePoint(new Coordinate(8, 60)));
        var act = () => BackcountrySkiActivity.Create(core, Details());
        act.Should().Throw<ArgumentException>().WithMessage("*route-based*");
    }

    [Fact]
    public void Create_rejects_negative_ascent()
    {
        var core = ActivityCore.New(Guid.NewGuid(), "n", null, Route());
        var act = () => BackcountrySkiActivity.Create(core, Details(ascentMeters: -10));
        act.Should().Throw<ArgumentException>().WithMessage("*non-negative*");
    }

    [Fact]
    public void Create_rejects_max_lower_than_min_elevation()
    {
        var core = ActivityCore.New(Guid.NewGuid(), "n", null, Route());
        var act = () => BackcountrySkiActivity.Create(core, Details(elevationMinMeters: 1500, elevationMaxMeters: 900));
        act.Should().Throw<ArgumentException>().WithMessage("*elevationMaxMeters*");
    }

    [Fact]
    public void Create_rejects_aspect_mix_not_summing_to_one()
    {
        var core = ActivityCore.New(Guid.NewGuid(), "n", null, Route());
        var bad = new[]
        {
            new AspectShare(Aspect.N, 0.3f),
            new AspectShare(Aspect.NE, 0.3f),
        };
        var act = () => BackcountrySkiActivity.Create(core, Details(aspectMix: bad));
        act.Should().Throw<ArgumentException>().WithMessage("*sum to 1.0*");
    }

    [Fact]
    public void Create_accepts_aspect_mix_summing_to_one()
    {
        var core = ActivityCore.New(Guid.NewGuid(), "n", null, Route());
        var mix = new[]
        {
            new AspectShare(Aspect.N, 0.6f),
            new AspectShare(Aspect.NW, 0.4f),
        };
        var act = () => BackcountrySkiActivity.Create(core, Details(aspectMix: mix));
        act.Should().NotThrow();
    }

    [Fact]
    public void Rename_increments_version_keeps_details()
    {
        var core = ActivityCore.New(Guid.NewGuid(), "Spot", null, Route());
        var bcski = BackcountrySkiActivity.Create(core, Details());
        var renamed = bcski.Rename("Different name", "new desc");
        renamed.Core.Name.Should().Be("Different name");
        renamed.Core.Version.Should().Be(2);
        renamed.Details.Should().BeSameAs(bcski.Details);
    }

    [Fact]
    public void SoftDelete_sets_deletedAt()
    {
        var core = ActivityCore.New(Guid.NewGuid(), "n", null, Route());
        var bcski = BackcountrySkiActivity.Create(core, Details());
        var deleted = bcski.SoftDelete();
        deleted.Core.DeletedAt.Should().NotBeNull();
    }
}
