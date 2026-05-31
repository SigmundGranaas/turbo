using System.Text.Json.Serialization;

namespace Turboapi.Activities.Packrafting.value;

/// <summary>
/// Post-run observation. Calibrates the orchestrator's flow-vs-grade
/// model: observed grade vs the route's stored typical grade, water
/// temperature actually felt, portages encountered, hazards.
/// </summary>
public sealed record PackraftingObservation
{
    [JsonPropertyName("observedGrade")] public string? ObservedGrade { get; init; }
    [JsonPropertyName("waterTempC")] public double? WaterTempC { get; init; }
    [JsonPropertyName("flowCumecs")] public double? FlowCumecs { get; init; }
    [JsonPropertyName("portagesTaken")] public int? PortagesTaken { get; init; }
    [JsonPropertyName("hazardsNoted")] public IReadOnlyList<string> HazardsNoted { get; init; }
    [JsonPropertyName("concerns")] public string? Concerns { get; init; }

    [JsonConstructor]
    public PackraftingObservation(
        string? observedGrade, double? waterTempC, double? flowCumecs,
        int? portagesTaken, IReadOnlyList<string>? hazardsNoted, string? concerns)
    {
        ObservedGrade = observedGrade;
        WaterTempC = waterTempC;
        FlowCumecs = flowCumecs;
        PortagesTaken = portagesTaken;
        HazardsNoted = hazardsNoted ?? Array.Empty<string>();
        Concerns = concerns;
    }
}
