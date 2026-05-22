using System.Text.Json.Serialization;

namespace Turboapi.Activities.Fishing.value;

/// <summary>
/// Typed payload of fishing-specific fields on the wire. No JSON catch-all:
/// every field is named and typed, both at rest in Postgres and on the
/// wire in events / API responses.
/// </summary>
public sealed record FishingDetails
{
    [JsonPropertyName("waterKind")]
    public WaterKind WaterKind { get; init; }

    [JsonPropertyName("shoreOrBoat")]
    public ShoreOrBoat ShoreOrBoat { get; init; }

    [JsonPropertyName("accessNotes")]
    public string? AccessNotes { get; init; }

    [JsonPropertyName("targetSpecies")]
    public IReadOnlyList<TargetSpecies> TargetSpecies { get; init; } = Array.Empty<TargetSpecies>();

    [JsonPropertyName("knownDepths")]
    public IReadOnlyList<DepthSample> KnownDepths { get; init; } = Array.Empty<DepthSample>();

    [JsonPropertyName("preferred")]
    public PreferredConditions? Preferred { get; init; }

    [JsonConstructor]
    public FishingDetails(
        WaterKind waterKind,
        ShoreOrBoat shoreOrBoat,
        string? accessNotes,
        IReadOnlyList<TargetSpecies>? targetSpecies,
        IReadOnlyList<DepthSample>? knownDepths,
        PreferredConditions? preferred)
    {
        WaterKind = waterKind;
        ShoreOrBoat = shoreOrBoat;
        AccessNotes = accessNotes;
        TargetSpecies = targetSpecies ?? Array.Empty<TargetSpecies>();
        KnownDepths = knownDepths ?? Array.Empty<DepthSample>();
        Preferred = preferred;
    }
}

public sealed record TargetSpecies
{
    [JsonPropertyName("speciesCode")]
    public string SpeciesCode { get; init; }

    [JsonPropertyName("notes")]
    public string? Notes { get; init; }

    [JsonConstructor]
    public TargetSpecies(string speciesCode, string? notes)
    {
        SpeciesCode = speciesCode ?? throw new ArgumentNullException(nameof(speciesCode));
        Notes = notes;
    }
}

public sealed record DepthSample
{
    [JsonPropertyName("lat")] public double Lat { get; init; }
    [JsonPropertyName("lon")] public double Lon { get; init; }
    [JsonPropertyName("depthMeters")] public float DepthMeters { get; init; }

    [JsonConstructor]
    public DepthSample(double lat, double lon, float depthMeters)
    {
        Lat = lat; Lon = lon; DepthMeters = depthMeters;
    }
}

/// <summary>
/// Optional user-attested "good conditions" for this spot. The fishing
/// conditions advisor uses these to compute a score against current
/// weather/tides.
/// </summary>
public sealed record PreferredConditions
{
    [JsonPropertyName("pressureMinHpa")] public short? PressureMinHpa { get; init; }
    [JsonPropertyName("pressureMaxHpa")] public short? PressureMaxHpa { get; init; }
    [JsonPropertyName("windMaxMs")] public float? WindMaxMs { get; init; }

    [JsonConstructor]
    public PreferredConditions(short? pressureMinHpa, short? pressureMaxHpa, float? windMaxMs)
    {
        PressureMinHpa = pressureMinHpa;
        PressureMaxHpa = pressureMaxHpa;
        WindMaxMs = windMaxMs;
    }
}
