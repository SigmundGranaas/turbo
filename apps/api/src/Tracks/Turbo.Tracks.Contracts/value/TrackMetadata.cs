using System.Text.Json.Serialization;

namespace Turboapi.Tracks.domain.value;

/// <summary>
/// Display metadata for a track. Mirrors the shape the Flutter
/// SavedPath model already persists locally (name, description, colour,
/// icon, line-style, smoothing flag) so the API contract maps 1:1.
/// </summary>
public record TrackMetadata
{
    [JsonPropertyName("name")]
    public string Name { get; init; }

    [JsonPropertyName("description")]
    public string? Description { get; init; }

    [JsonPropertyName("colorHex")]
    public string? ColorHex { get; init; }

    [JsonPropertyName("iconKey")]
    public string? IconKey { get; init; }

    [JsonPropertyName("lineStyleKey")]
    public string? LineStyleKey { get; init; }

    [JsonPropertyName("smoothing")]
    public bool Smoothing { get; init; }

    [JsonConstructor]
    public TrackMetadata(
        string name,
        string? description = null,
        string? colorHex = null,
        string? iconKey = null,
        string? lineStyleKey = null,
        bool smoothing = false)
    {
        Name = name;
        Description = description;
        ColorHex = colorHex;
        IconKey = iconKey;
        LineStyleKey = lineStyleKey;
        Smoothing = smoothing;
    }

    public TrackMetadata()
    {
        Name = string.Empty;
    }

    public static TrackMetadata Empty => new("");
}

/// <summary>
/// Optional changes to a track's <see cref="TrackMetadata"/>. Each nullable
/// field signals "no change requested" when null; a non-null value
/// replaces the current value.
/// </summary>
public record TrackMetadataUpdate
{
    [JsonPropertyName("name")]
    public string? Name { get; init; }

    [JsonPropertyName("description")]
    public string? Description { get; init; }

    [JsonPropertyName("colorHex")]
    public string? ColorHex { get; init; }

    [JsonPropertyName("iconKey")]
    public string? IconKey { get; init; }

    [JsonPropertyName("lineStyleKey")]
    public string? LineStyleKey { get; init; }

    [JsonPropertyName("smoothing")]
    public bool? Smoothing { get; init; }

    [JsonIgnore]
    public bool HasAnyChange =>
        Name is not null ||
        Description is not null ||
        ColorHex is not null ||
        IconKey is not null ||
        LineStyleKey is not null ||
        Smoothing is not null;

    [JsonConstructor]
    public TrackMetadataUpdate(
        string? name = null,
        string? description = null,
        string? colorHex = null,
        string? iconKey = null,
        string? lineStyleKey = null,
        bool? smoothing = null)
    {
        Name = name;
        Description = description;
        ColorHex = colorHex;
        IconKey = iconKey;
        LineStyleKey = lineStyleKey;
        Smoothing = smoothing;
    }
}
