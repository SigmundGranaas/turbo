using System.Text.Json.Nodes;
using FluentAssertions;
using Turboapi.Places.Core;
using Xunit;

namespace Turbo.Places.Behaviour;

/// <summary>
/// Replays the shared place-core golden fixture through the .NET P/Invoke
/// surface — the server-side counterpart of the Python/Kotlin synthetic
/// clients. If this passes, the server ranks exactly like every other binding
/// of the core.
/// </summary>
public class PlaceCoreGoldenParity
{
    public PlaceCoreGoldenParity()
    {
        Environment.SetEnvironmentVariable("PLACE_CORE_LIB", PlacesHostFixture.FindPlaceCoreLibDir());
    }

    [Fact]
    public void Every_golden_reverse_case_matches_through_PInvoke()
    {
        var goldenPath = Path.Combine(
            PlacesHostFixture.FindRepoRoot(), "packages", "place-core", "golden.json");
        var cases = JsonNode.Parse(File.ReadAllText(goldenPath))!.AsArray();
        cases.Count.Should().BeGreaterThan(20, "the golden fixture is the behavioural contract");

        var failures = new List<string>();
        foreach (var c in cases)
        {
            var name = c!["name"]!.GetValue<string>();
            // The fixture's input/expect are already in the cabi's serde
            // (snake_case) shape — pass them through verbatim.
            var got = JsonNode.Parse(PlaceCore.ReverseJson(c["input"]!.ToJsonString()));
            var want = c["expect"];
            if (!JsonNode.DeepEquals(got, want))
                failures.Add($"{name}\n  want: {want?.ToJsonString() ?? "null"}\n  got:  {got?.ToJsonString() ?? "null"}");
        }

        failures.Should().BeEmpty(
            "the .NET binding must rank identically to the Rust core:\n{0}",
            string.Join("\n\n", failures));
    }
}
