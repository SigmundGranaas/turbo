using System.Text.Json.Serialization;

namespace Turboapi.Geo.domain.value;

public record LocationUpdateParameters
{
    /// <summary>
    /// If not null, new coordinates to set.
    /// </summary>
    [JsonPropertyName("coordinates")]
    public Coordinates? Coordinates { get; init; }

    /// <summary>
    /// If not null, contains desired changes for display information.
    /// </summary>
    [JsonPropertyName("display")]
    public DisplayUpdate? Display { get; init; }

    [JsonConstructor]
    public LocationUpdateParameters(Coordinates? coordinates = null, DisplayUpdate? display = null)
    {
        Coordinates = coordinates;
        Display = display;
    }

    /// <summary>
    /// Checks if this update specification proposes any actual change.
    /// </summary>
    [JsonIgnore] // This property should not be part of serialization
    public bool HasAnyChange => Coordinates != null || (Display != null && Display.HasAnyChange);
}