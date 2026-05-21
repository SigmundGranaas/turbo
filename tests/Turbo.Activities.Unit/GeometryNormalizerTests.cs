using FluentAssertions;
using NetTopologySuite.Geometries;
using Turboapi.Activities.domain.services;
using Turboapi.Activities.value;
using Xunit;

namespace Turbo.Activities.Unit;

public sealed class GeometryNormalizerTests
{
    private static readonly GeometryFactory F = new(new PrecisionModel(), 4326);
    private readonly GeometryNormalizer _norm = new();

    [Fact]
    public void Stamps_srid_when_missing()
    {
        var p = new GeometryFactory().CreatePoint(new Coordinate(5, 60)); // no SRID
        var result = _norm.Normalize(p, ActivityGeometryKind.Point);
        result.SRID.Should().Be(4326);
    }

    [Fact]
    public void Rejects_wrong_geometry_kind()
    {
        var ls = F.CreateLineString(new[] { new Coordinate(0, 0), new Coordinate(1, 1) });
        var act = () => _norm.Normalize(ls, ActivityGeometryKind.Point);
        act.Should().Throw<ArgumentException>();
    }

    [Fact]
    public void Rejects_linestring_with_fewer_than_two_points()
    {
        // A 1-point line is technically invalid in NTS too; build via empty
        var emptyLs = F.CreateLineString(Array.Empty<Coordinate>());
        var act = () => _norm.Normalize(emptyLs, ActivityGeometryKind.LineString);
        act.Should().Throw<ArgumentException>();
    }

    [Fact]
    public void Rejects_out_of_range_longitude()
    {
        var p = F.CreatePoint(new Coordinate(200, 0));
        var act = () => _norm.Normalize(p, ActivityGeometryKind.Point);
        act.Should().Throw<ArgumentException>();
    }

    [Fact]
    public void Accepts_valid_polygon()
    {
        var ring = F.CreateLinearRing(new[]
        {
            new Coordinate(0, 0), new Coordinate(0, 1), new Coordinate(1, 1),
            new Coordinate(1, 0), new Coordinate(0, 0),
        });
        var poly = F.CreatePolygon(ring);
        var result = _norm.Normalize(poly, ActivityGeometryKind.Polygon);
        result.Should().BeSameAs(poly);
    }
}
