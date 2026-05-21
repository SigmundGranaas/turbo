using System.Text;
using FluentAssertions;
using Turboapi.Activities.conditions;
using Xunit;

namespace Turbo.Activities.Unit;

/// <summary>
/// Verifies the XML parser tolerates the real Sehavnivå tideapi.php
/// shape — observed and forecast waterlevel entries with attributes
/// in varying orders, ISO timestamps with offsets, decimal values
/// with a leading minus or zero. We do not test the HTTP layer here
/// (that's the wrapper around IHttpClientFactory); we test the slice
/// extraction directly via <c>ParseWaterlevels</c>, which is exposed
/// internal-to-tests for this reason.
/// </summary>
public sealed class SehavnivaTideProviderTests
{
    [Fact]
    public void Parses_typical_response_shape()
    {
        var xml = """
            <?xml version="1.0"?>
            <tide>
              <locationdata>
                <data>
                  <waterlevel value="0.12" time="2026-05-20T11:50:00+00:00" flag="obs" />
                  <waterlevel time="2026-05-20T12:00:00+00:00" value="0.18" flag="obs" />
                  <waterlevel value="0.22" time="2026-05-20T12:10:00+00:00" flag="pred" />
                </data>
              </locationdata>
            </tide>
            """;
        using var stream = new MemoryStream(Encoding.UTF8.GetBytes(xml));
        var points = SehavnivaTideProvider.ParseWaterlevels(stream);

        points.Should().HaveCount(3);
        points[0].Value.Should().BeApproximately(0.12f, 0.0001f);
        points[1].Value.Should().BeApproximately(0.18f, 0.0001f);
        points[2].Value.Should().BeApproximately(0.22f, 0.0001f);
        points[0].Time.Should().Be(new DateTimeOffset(2026, 5, 20, 11, 50, 0, TimeSpan.Zero));
    }

    [Fact]
    public void Skips_entries_with_missing_attributes()
    {
        var xml = """
            <?xml version="1.0"?>
            <tide>
              <locationdata>
                <data>
                  <waterlevel value="0.12" time="2026-05-20T11:50:00+00:00" />
                  <waterlevel flag="obs" />
                  <waterlevel value="not-a-number" time="2026-05-20T12:10:00+00:00" />
                </data>
              </locationdata>
            </tide>
            """;
        using var stream = new MemoryStream(Encoding.UTF8.GetBytes(xml));
        var points = SehavnivaTideProvider.ParseWaterlevels(stream);

        points.Should().HaveCount(1);
        points[0].Value.Should().BeApproximately(0.12f, 0.0001f);
    }

    [Fact]
    public void Handles_negative_levels_and_offsets()
    {
        var xml = """
            <?xml version="1.0"?>
            <tide><locationdata><data>
              <waterlevel value="-0.42" time="2026-05-20T13:50:00+02:00" />
            </data></locationdata></tide>
            """;
        using var stream = new MemoryStream(Encoding.UTF8.GetBytes(xml));
        var points = SehavnivaTideProvider.ParseWaterlevels(stream);

        points.Should().HaveCount(1);
        points[0].Value.Should().BeApproximately(-0.42f, 0.0001f);
        // 13:50 +02:00 → 11:50 UTC
        points[0].Time.Should().Be(new DateTimeOffset(2026, 5, 20, 11, 50, 0, TimeSpan.Zero));
    }
}
