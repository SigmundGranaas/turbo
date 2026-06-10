using System.Net.Http.Json;
using System.Text.Json;
using FluentAssertions;
using Xunit;

namespace Turbo.Places.Behaviour;

/// <summary>
/// Search must surface protected areas and kommuner by name, not just toponyms
/// — a user searching "Sjunkhatten" expects the national park. The fixture
/// seeds the Jotunheimen park + Lom kommune.
/// </summary>
public class PlacesAreaSearchBehaviour : IClassFixture<PlacesHostFixture>
{
    private readonly PlacesHostFixture _fixture;

    public PlacesAreaSearchBehaviour(PlacesHostFixture fixture) => _fixture = fixture;

    [Fact]
    public async Task Search_finds_a_protected_area_by_name()
    {
        var client = _fixture.CreateClient();
        var body = await client.GetFromJsonAsync<JsonElement>(
            "/api/places/search?q=jotun&lat=61.5&lon=8.4&limit=5");

        var titles = body.GetProperty("items").EnumerateArray()
            .Select(i => i.GetProperty("title").GetString()).ToList();
        titles.Should().Contain("Jotunheimen", "the national park is searchable by name");

        var park = body.GetProperty("items").EnumerateArray()
            .First(i => i.GetProperty("title").GetString() == "Jotunheimen");
        park.GetProperty("icon").GetString().Should().Be("park");
        park.GetProperty("description").GetString().Should().Contain("Nasjonalpark");
    }

    [Fact]
    public async Task Search_finds_a_kommune_by_name()
    {
        var client = _fixture.CreateClient();
        var body = await client.GetFromJsonAsync<JsonElement>(
            "/api/places/search?q=lom&lat=61.5&lon=8.4&limit=10");

        body.GetProperty("items").EnumerateArray()
            .Select(i => i.GetProperty("title").GetString())
            .Should().Contain("Lom", "the kommune is searchable by name");
    }
}
