using FluentAssertions;
using NetTopologySuite.Geometries;
using Turboapi.Activities.domain;
using Xunit;

namespace Turbo.Activities.Unit;

public sealed class ActivityCoreTests
{
    private static Point P(double lon, double lat) =>
        new GeometryFactory(new PrecisionModel(), 4326).CreatePoint(new Coordinate(lon, lat));

    [Fact]
    public void New_assigns_identity_and_v1()
    {
        var owner = Guid.NewGuid();
        var core = ActivityCore.New(owner, "Spot A", "desc", P(5, 60));

        core.Id.Should().NotBeEmpty();
        core.OwnerId.Should().Be(owner);
        core.Name.Should().Be("Spot A");
        core.Description.Should().Be("desc");
        core.Version.Should().Be(1);
        core.DeletedAt.Should().BeNull();
        core.CreatedAt.Should().BeCloseTo(DateTime.UtcNow, TimeSpan.FromSeconds(5));
        core.UpdatedAt.Should().Be(core.CreatedAt);
    }

    [Fact]
    public void New_rejects_empty_owner()
    {
        var act = () => ActivityCore.New(Guid.Empty, "n", null, P(0, 0));
        act.Should().Throw<ArgumentException>().WithParameterName("ownerId");
    }

    [Fact]
    public void New_rejects_blank_name()
    {
        var act = () => ActivityCore.New(Guid.NewGuid(), "  ", null, P(0, 0));
        act.Should().Throw<ArgumentException>().WithParameterName("name");
    }

    [Fact]
    public void WithRename_increments_version_and_updatedAt()
    {
        var c1 = ActivityCore.New(Guid.NewGuid(), "Spot A", null, P(0, 0));
        Thread.Sleep(2);
        var c2 = c1.WithRename("Spot B", "new desc");

        c2.Name.Should().Be("Spot B");
        c2.Description.Should().Be("new desc");
        c2.Version.Should().Be(2);
        c2.UpdatedAt.Should().BeAfter(c1.UpdatedAt);
        c2.Id.Should().Be(c1.Id);
    }

    [Fact]
    public void WithRename_blank_name_keeps_previous()
    {
        var c1 = ActivityCore.New(Guid.NewGuid(), "Spot A", null, P(0, 0));
        var c2 = c1.WithRename("   ", null);
        c2.Name.Should().Be("Spot A");
    }

    [Fact]
    public void WithSoftDelete_sets_deletedAt_and_bumps_version()
    {
        var c1 = ActivityCore.New(Guid.NewGuid(), "n", null, P(0, 0));
        var c2 = c1.WithSoftDelete();

        c2.DeletedAt.Should().NotBeNull();
        c2.DeletedAt.Should().BeCloseTo(DateTime.UtcNow, TimeSpan.FromSeconds(5));
        c2.Version.Should().Be(2);
    }
}
