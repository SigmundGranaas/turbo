using System.Text.Json.Serialization;

namespace Turboapi.Activities.value;

/// <summary>
/// Typed tide / sea-state snapshot at a location. <c>Summary</c> is a
/// short string the client renders verbatim — "rising 1h to high
/// tide", "slack", "spring outflow", etc.
/// </summary>
public sealed record TideSlice
{
    [JsonPropertyName("validAt")] public DateTimeOffset ValidAt { get; init; }
    [JsonPropertyName("currentHeightMeters")] public float? CurrentHeightMeters { get; init; }
    [JsonPropertyName("summary")] public string? Summary { get; init; }

    [JsonConstructor]
    public TideSlice(DateTimeOffset validAt, float? currentHeightMeters, string? summary)
    {
        ValidAt = validAt;
        CurrentHeightMeters = currentHeightMeters;
        Summary = summary;
    }
}

/// <summary>
/// Tide / sea-state source. Freediving's advisor consumes this; sea
/// fishing will too. Implementations live in Shared.Infrastructure.
/// </summary>
public interface ITideProvider
{
    string Key { get; }

    Task<TideSlice> GetAsync(
        double latitude, double longitude,
        DateTimeOffset at,
        CancellationToken cancellationToken);
}
