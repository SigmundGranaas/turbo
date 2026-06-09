using System.Text.Json;

namespace Turboapi.Places.Core;

/// <summary>
/// Serves the place-core ruleset artifact (for <c>GET /api/places/ruleset/
/// {version}</c> and, later, bundle embedding). Reads it once from the native
/// core so server and core never drift.
/// </summary>
public sealed class RulesetProvider
{
    public string Json { get; }
    public string Version { get; }

    public RulesetProvider()
    {
        Json = PlaceCore.RulesetJson();
        using var doc = JsonDocument.Parse(Json);
        Version = doc.RootElement.TryGetProperty("version", out var v)
            ? v.GetString() ?? "unknown"
            : "unknown";
    }

    /// <summary>The ruleset JSON for <paramref name="version"/>, or null if we
    /// don't have that version. (Only the embedded version exists today;
    /// ingested versions join when ruleset evolution lands.)</summary>
    public string? ForVersion(string version) =>
        version == Version ? Json : null;
}
