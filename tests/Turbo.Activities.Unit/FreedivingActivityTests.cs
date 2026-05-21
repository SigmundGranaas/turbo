using FluentAssertions;
using NetTopologySuite.Geometries;
using Turboapi.Activities.domain;
using Turboapi.Activities.Freediving.domain;
using Turboapi.Activities.Freediving.value;
using Xunit;

namespace Turbo.Activities.Unit;

public sealed class FreedivingActivityTests
{
    private static readonly GeometryFactory F = new(new PrecisionModel(), 4326);

    private static FreedivingDetails Details(float maxDepth = 12.0f) => new(
        waterBody: WaterBody.Sea, bottomType: BottomType.KelpForest,
        maxDepthMeters: maxDepth,
        typicalVisibilityMeters: 5.0f,
        harpoonAllowed: true, shoreEntry: true,
        accessNotes: null, targetSpecies: null);

    [Fact]
    public void Create_with_Point_succeeds()
    {
        var core = ActivityCore.New(Guid.NewGuid(), "Spot", null, F.CreatePoint(new Coordinate(5.3, 60.4)));
        var f = FreedivingActivity.Create(core, Details());
        f.Details.WaterBody.Should().Be(WaterBody.Sea);
        f.Details.HarpoonAllowed.Should().BeTrue();
    }

    [Fact]
    public void Create_rejects_linestring()
    {
        var line = F.CreateLineString(new[] { new Coordinate(0, 0), new Coordinate(1, 1) });
        var core = ActivityCore.New(Guid.NewGuid(), "n", null, line);
        var act = () => FreedivingActivity.Create(core, Details());
        act.Should().Throw<ArgumentException>().WithMessage("*spot-based*");
    }

    [Fact]
    public void Create_rejects_implausibly_deep_spot()
    {
        var core = ActivityCore.New(Guid.NewGuid(), "n", null, F.CreatePoint(new Coordinate(5, 60)));
        var act = () => FreedivingActivity.Create(core, Details(maxDepth: 250));
        act.Should().Throw<ArgumentException>().WithMessage("*implausible*");
    }
}
