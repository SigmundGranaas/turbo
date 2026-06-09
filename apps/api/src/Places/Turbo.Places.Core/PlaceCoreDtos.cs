using System.Text.Json;
using System.Text.Json.Serialization;

namespace Turboapi.Places.Core;

/// <summary>
/// JSON DTOs matching the place-core C-ABI contract field-for-field (serde
/// snake_case; qualifier is the serde-renamed string e.g. "on"/"closeTo").
/// </summary>
public static class PlaceCoreJson
{
    public static readonly JsonSerializerOptions Options = new()
    {
        DefaultIgnoreCondition = JsonIgnoreCondition.WhenWritingNull,
    };
}

public sealed class ReverseInputDto
{
    [JsonPropertyName("toponyms")] public List<CandidateDto> Toponyms { get; set; } = new();
    // protected_area / address / kommune / elevation_m are optional in
    // place-core (serde default); omitted here until those sources are ingested.
}

public sealed class CandidateDto
{
    [JsonPropertyName("name")] public string Name { get; set; } = "";
    [JsonPropertyName("kind")] public string Kind { get; set; } = "";
    [JsonPropertyName("distance_m")] public double DistanceM { get; set; }
    [JsonPropertyName("status")] public string? Status { get; set; }
}

public sealed class LocationDescriptionDto
{
    [JsonPropertyName("title")] public string Title { get; set; } = "";
    [JsonPropertyName("qualifier")] public string? Qualifier { get; set; }
    [JsonPropertyName("secondary")] public string? Secondary { get; set; }
    [JsonPropertyName("kommune")] public string? Kommune { get; set; }
    [JsonPropertyName("fylke")] public string? Fylke { get; set; }
    [JsonPropertyName("distance_m")] public double? DistanceM { get; set; }
    [JsonPropertyName("elevation_m")] public double? ElevationM { get; set; }
}
