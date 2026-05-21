using Turboapi.Activities.value;

namespace Turboapi.Activities.conditions;

/// <summary>
/// Deterministic avalanche-forecast generator. Same (region, day)
/// always returns the same slice. Used when the real
/// <see cref="VarsomAvalancheProvider"/> isn't wired up (dev / test /
/// staging without network). Output is plausible enough to drive the
/// kind's advisor + UI but is NOT a substitute for the real Varsom
/// bulletin — the bcski advisor's rationale already nudges users to
/// check Varsom directly.
/// </summary>
public sealed class SyntheticAvalancheProvider : IAvalancheProvider
{
    public string Key => "synthetic_avalanche";

    public Task<AvalancheSlice> GetAsync(
        int varsomRegionId, DateTimeOffset at, CancellationToken cancellationToken)
    {
        var day = new DateTimeOffset(at.Year, at.Month, at.Day, 0, 0, 0, TimeSpan.Zero);
        var seed = HashCode.Combine(varsomRegionId, day.ToUnixTimeSeconds());
        var rng = new Random(seed);

        // Bias toward 2–3 (the realistic everyday Norwegian winter range).
        var roll = rng.NextDouble();
        var level = roll switch
        {
            < 0.10 => 1,
            < 0.40 => 2,
            < 0.85 => 3,
            < 0.97 => 4,
            _ => 5,
        };

        var problems = level switch
        {
            1 => "",
            2 => rng.Next(2) == 0 ? "WindSlab" : "PersistentSlab",
            3 => "WindSlab,PersistentSlab",
            4 => "WindSlab,PersistentSlab,WetSnow",
            _ => "WindSlab,PersistentSlab,WetSnow,Cornice",
        };

        var summary = level switch
        {
            1 => "Low. Stable conditions in most terrain.",
            2 => "Moderate. Heightened avalanche conditions on specific terrain features.",
            3 => "Considerable. Dangerous avalanche conditions; careful snowpack evaluation essential.",
            4 => "High. Very dangerous conditions; travel in avalanche terrain not recommended.",
            _ => "Extreme. Avoid all avalanche terrain.",
        };

        return Task.FromResult(new AvalancheSlice(day, level, summary, problems));
    }
}
