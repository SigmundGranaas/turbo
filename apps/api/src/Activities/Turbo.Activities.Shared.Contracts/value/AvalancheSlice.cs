using System.Text.Json.Serialization;

namespace Turboapi.Activities.value;

/// <summary>
/// Typed avalanche-forecast slice, modelled on Varsom's region-day
/// schema. Server-cached per (region, day-bucket); the bcski advisor
/// composes this with the WeatherSlice into its typed report.
/// </summary>
public sealed record AvalancheSlice
{
    [JsonPropertyName("validFor")] public DateTimeOffset ValidFor { get; init; }

    /// <summary>1–5 European avalanche danger scale.</summary>
    [JsonPropertyName("dangerLevel")] public int DangerLevel { get; init; }

    /// <summary>Short headline ("Considerable above 1200m, slab problem on N–NE").</summary>
    [JsonPropertyName("summary")] public string Summary { get; init; }

    /// <summary>Comma-separated problem codes from the Varsom taxonomy
    /// (e.g. "WindSlab,WetSnow"). Empty when no problem dominates.</summary>
    [JsonPropertyName("problems")] public string Problems { get; init; }

    [JsonConstructor]
    public AvalancheSlice(DateTimeOffset validFor, int dangerLevel, string summary, string problems)
    {
        ValidFor = validFor;
        DangerLevel = dangerLevel;
        Summary = summary ?? throw new ArgumentNullException(nameof(summary));
        Problems = problems ?? throw new ArgumentNullException(nameof(problems));
    }
}

/// <summary>
/// Avalanche-forecast source for one Varsom region. Implementations
/// live in Shared.Infrastructure (HTTP + synthetic). Defined here so
/// the bcski advisor can compose against the typed contract.
/// </summary>
public interface IAvalancheProvider
{
    string Key { get; }
    Task<AvalancheSlice> GetAsync(
        int varsomRegionId,
        DateTimeOffset at,
        CancellationToken cancellationToken);
}
