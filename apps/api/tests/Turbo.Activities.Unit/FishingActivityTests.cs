using FluentAssertions;
using NetTopologySuite.Geometries;
using Turboapi.Activities.domain;
using Turboapi.Activities.Fishing.domain;
using Turboapi.Activities.Fishing.value;
using Xunit;

namespace Turbo.Activities.Unit;

public sealed class FishingActivityTests
{
    private static readonly GeometryFactory F = new(new PrecisionModel(), 4326);

    private static FishingDetails MinimalDetails(WaterKind water = WaterKind.River) =>
        new(water, ShoreOrBoat.Either, null, null, null, null);

    [Fact]
    public void Create_with_point_succeeds()
    {
        var core = ActivityCore.New(Guid.NewGuid(), "Spot", null, F.CreatePoint(new Coordinate(5, 60)));
        var fa = FishingActivity.Create(core, MinimalDetails());
        fa.Core.Should().BeSameAs(core);
        fa.Details.WaterKind.Should().Be(WaterKind.River);
    }

    [Fact]
    public void Create_rejects_non_point_geometry()
    {
        var line = F.CreateLineString(new[] { new Coordinate(0, 0), new Coordinate(1, 1) });
        var core = ActivityCore.New(Guid.NewGuid(), "Spot", null, line);

        var act = () => FishingActivity.Create(core, MinimalDetails());
        act.Should().Throw<ArgumentException>().WithMessage("*point-based*");
    }

    [Fact]
    public void Rename_returns_new_fishing_with_bumped_core_version()
    {
        var core = ActivityCore.New(Guid.NewGuid(), "Spot", null, F.CreatePoint(new Coordinate(5, 60)));
        var fa = FishingActivity.Create(core, MinimalDetails());

        var fa2 = fa.Rename("Renamed", "desc");

        fa2.Core.Name.Should().Be("Renamed");
        fa2.Core.Description.Should().Be("desc");
        fa2.Core.Version.Should().Be(2);
        fa2.Details.Should().BeSameAs(fa.Details);
    }

    [Fact]
    public void ReplaceDetails_returns_new_fishing_with_new_details_and_bumped_version()
    {
        var core = ActivityCore.New(Guid.NewGuid(), "Spot", null, F.CreatePoint(new Coordinate(5, 60)));
        var fa = FishingActivity.Create(core, MinimalDetails(WaterKind.River));

        var fa2 = fa.ReplaceDetails(MinimalDetails(WaterKind.Sea));

        fa2.Details.WaterKind.Should().Be(WaterKind.Sea);
        fa2.Core.Version.Should().Be(2);
        fa2.Core.Id.Should().Be(fa.Core.Id);
    }

    [Fact]
    public void SoftDelete_sets_core_deletedAt()
    {
        var core = ActivityCore.New(Guid.NewGuid(), "Spot", null, F.CreatePoint(new Coordinate(5, 60)));
        var fa = FishingActivity.Create(core, MinimalDetails());

        var fa2 = fa.SoftDelete();
        fa2.Core.DeletedAt.Should().NotBeNull();
    }
}
