using System.Text.Json;

namespace Turboapi.Places.Core;

/// <summary>
/// Boot-time verification that the place-core native library loads and ranks
/// correctly. Run at host startup so a missing/mismatched <c>libplace_core</c>
/// fails the deploy with an actionable message, instead of booting healthy and
/// 500-ing on the first request.
/// </summary>
public static class PlacesStartupProbe
{
    // A peak 30 m away → "On <peak>" under any valid ruleset (golden case #1).
    private const string ProbeInput =
        """{"toponyms":[{"name":"Galdhøpiggen","kind":"Fjelltopp","distance_m":30.0,"status":"aktiv"}]}""";

    public static void Verify() => Verify(PlaceCore.ReverseJson);

    /// <summary>Testable seam: <paramref name="reverseJson"/> is the core call.</summary>
    public static void Verify(Func<string, string> reverseJson)
    {
        string resultJson;
        try
        {
            resultJson = reverseJson(ProbeInput);
        }
        catch (Exception ex)
        {
            throw new InvalidOperationException(
                "place-core native library (libplace_core) could not be loaded. " +
                "Build it with `cargo build --features cabi` and point PLACE_CORE_LIB " +
                "at the directory containing libplace_core.so (or stage it next to the host).",
                ex);
        }

        LocationDescriptionDto? d;
        try { d = JsonSerializer.Deserialize<LocationDescriptionDto>(resultJson, PlaceCoreJson.Options); }
        catch (JsonException ex)
        {
            throw new InvalidOperationException(
                $"place-core probe returned unparseable output: {resultJson}", ex);
        }

        if (d?.Title != "Galdhøpiggen" || d.Qualifier != "on")
        {
            throw new InvalidOperationException(
                "place-core probe returned an unexpected verdict " +
                $"(title={d?.Title}, qualifier={d?.Qualifier}) — native library or ruleset mismatch.");
        }
    }
}
