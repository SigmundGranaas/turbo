using System.Text.Json.Serialization;

namespace Turboapi.Geo.domain.value;

/// <summary>
/// Immutable display information record
/// </summary>
public record DisplayInformation
{
    [JsonPropertyName("name")]
    public string Name { get; init; }

    [JsonPropertyName("description")]
    public string Description { get; init; }

    [JsonPropertyName("icon")]
    public string Icon { get; init; }

    [JsonConstructor]
    public DisplayInformation(string name, string description = "", string icon = "")
    {
        Name = name;
        Description = description;
        Icon = icon;
    }
    // Parameterless constructor if needed by other frameworks or as a fallback.
    public DisplayInformation()
    {
        Name = "";       // Initialize to default/empty values
        Description = "";
        Icon = "";
    }

    public static DisplayInformation Empty => new DisplayInformation("", "", "");
}