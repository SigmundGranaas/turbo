using System.Text.Json.Serialization;

namespace Turboapi.Collections.domain.value;

public record CollectionMetadata
{
    [JsonPropertyName("name")]
    public string Name { get; init; }

    [JsonPropertyName("description")]
    public string? Description { get; init; }

    [JsonPropertyName("colorHex")]
    public string? ColorHex { get; init; }

    [JsonPropertyName("iconKey")]
    public string? IconKey { get; init; }

    [JsonPropertyName("sortOrder")]
    public int SortOrder { get; init; }

    /// <summary>
    /// Opaque JSON blob describing a "smart" collection's filter
    /// criteria. The server treats it as a passthrough — clients
    /// interpret and evaluate it.
    /// </summary>
    [JsonPropertyName("savedFilter")]
    public string? SavedFilterJson { get; init; }

    [JsonConstructor]
    public CollectionMetadata(
        string name,
        string? description = null,
        string? colorHex = null,
        string? iconKey = null,
        int sortOrder = 0,
        string? savedFilterJson = null)
    {
        Name = name;
        Description = description;
        ColorHex = colorHex;
        IconKey = iconKey;
        SortOrder = sortOrder;
        SavedFilterJson = savedFilterJson;
    }

    public CollectionMetadata()
    {
        Name = string.Empty;
    }

    public static CollectionMetadata Empty => new("");
}

public record CollectionMetadataUpdate
{
    [JsonPropertyName("name")]
    public string? Name { get; init; }

    [JsonPropertyName("description")]
    public string? Description { get; init; }

    [JsonPropertyName("colorHex")]
    public string? ColorHex { get; init; }

    [JsonPropertyName("iconKey")]
    public string? IconKey { get; init; }

    [JsonPropertyName("sortOrder")]
    public int? SortOrder { get; init; }

    [JsonPropertyName("savedFilter")]
    public string? SavedFilterJson { get; init; }

    /// <summary>
    /// Sentinel: the client wants to clear the savedFilter (turn a smart
    /// collection back into a manual one). Distinct from "no change to
    /// savedFilter" because <see cref="SavedFilterJson"/> is nullable
    /// already.
    /// </summary>
    [JsonPropertyName("clearSavedFilter")]
    public bool ClearSavedFilter { get; init; }

    [JsonIgnore]
    public bool HasAnyChange =>
        Name is not null ||
        Description is not null ||
        ColorHex is not null ||
        IconKey is not null ||
        SortOrder is not null ||
        SavedFilterJson is not null ||
        ClearSavedFilter;

    [JsonConstructor]
    public CollectionMetadataUpdate(
        string? name = null,
        string? description = null,
        string? colorHex = null,
        string? iconKey = null,
        int? sortOrder = null,
        string? savedFilterJson = null,
        bool clearSavedFilter = false)
    {
        Name = name;
        Description = description;
        ColorHex = colorHex;
        IconKey = iconKey;
        SortOrder = sortOrder;
        SavedFilterJson = savedFilterJson;
        ClearSavedFilter = clearSavedFilter;
    }
}
