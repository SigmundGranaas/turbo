using System.Text.Json.Serialization;

namespace Turboapi.Geo.domain.value;

/// <summary>
/// Represents a set of proposed changes for DisplayInformation.
/// Null properties indicate no change is requested for that field.
/// </summary>
public record DisplayUpdate
{
    [JsonPropertyName("name")]
    public string? Name { get; init; }

    [JsonPropertyName("description")]
    public string? Description { get; init; }

    [JsonPropertyName("icon")]
    public string? Icon { get; init; }

    // Helper to check if any change is actually proposed by this specific update object
    [JsonIgnore] // This property should not be part of serialization
    public bool HasAnyChange => Name != null || Description != null || Icon != null;

    // Constructor for convenience
    [JsonConstructor]
    public DisplayUpdate(string? name = null, string? description = null, string? icon = null)
    {
        Name = name;
        Description = description;
        Icon = icon;
    }
}