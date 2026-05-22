using System.Text.Json.Serialization;

namespace Turboapi.Activities.Freediving.value;

public sealed record FreedivingDetails
{
    [JsonPropertyName("waterBody")] public WaterBody WaterBody { get; init; }
    [JsonPropertyName("bottomType")] public BottomType BottomType { get; init; }
    [JsonPropertyName("maxDepthMeters")] public float MaxDepthMeters { get; init; }
    [JsonPropertyName("typicalVisibilityMeters")] public float? TypicalVisibilityMeters { get; init; }
    [JsonPropertyName("harpoonAllowed")] public bool HarpoonAllowed { get; init; }
    [JsonPropertyName("shoreEntry")] public bool ShoreEntry { get; init; }
    [JsonPropertyName("accessNotes")] public string? AccessNotes { get; init; }
    [JsonPropertyName("targetSpecies")] public IReadOnlyList<TargetSpecies> TargetSpecies { get; init; } = Array.Empty<TargetSpecies>();

    [JsonConstructor]
    public FreedivingDetails(
        WaterBody waterBody, BottomType bottomType, float maxDepthMeters,
        float? typicalVisibilityMeters, bool harpoonAllowed, bool shoreEntry,
        string? accessNotes, IReadOnlyList<TargetSpecies>? targetSpecies)
    {
        WaterBody = waterBody;
        BottomType = bottomType;
        MaxDepthMeters = maxDepthMeters;
        TypicalVisibilityMeters = typicalVisibilityMeters;
        HarpoonAllowed = harpoonAllowed;
        ShoreEntry = shoreEntry;
        AccessNotes = accessNotes;
        TargetSpecies = targetSpecies ?? Array.Empty<TargetSpecies>();
    }
}

public sealed record TargetSpecies
{
    [JsonPropertyName("speciesCode")] public string SpeciesCode { get; init; }
    [JsonPropertyName("notes")] public string? Notes { get; init; }

    [JsonConstructor]
    public TargetSpecies(string speciesCode, string? notes)
    {
        SpeciesCode = speciesCode ?? throw new ArgumentNullException(nameof(speciesCode));
        Notes = notes;
    }
}
