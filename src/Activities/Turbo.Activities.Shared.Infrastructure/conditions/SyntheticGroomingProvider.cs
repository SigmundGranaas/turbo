using Turboapi.Activities.value;

namespace Turboapi.Activities.conditions;

/// <summary>
/// Deterministic grooming generator. Per-feed-key + per-day hash so the
/// same trail returns the same recency throughout the day. Plausible
/// distribution: most trails groomed within the last 24h during winter
/// months, rarely in summer.
/// </summary>
public sealed class SyntheticGroomingProvider : IGroomingProvider
{
    public string Key => "synthetic_grooming";

    public Task<GroomingSlice> GetAsync(
        string feedKey, DateTimeOffset at, CancellationToken cancellationToken)
    {
        var seed = HashCode.Combine(feedKey, at.Year * 1000 + at.DayOfYear);
        var rng = new Random(seed);
        var isWinter = at.Month is <= 4 or >= 11;
        var roll = rng.NextDouble();
        var hoursAgo = isWinter
            ? roll switch
            {
                < 0.5 => rng.Next(1, 12),
                < 0.85 => rng.Next(12, 36),
                _ => rng.Next(36, 96),
            }
            : rng.Next(120, 720); // summer: weeks ago
        return Task.FromResult(new GroomingSlice(
            validAt: at.AddHours(-hoursAgo),
            hoursAgo: hoursAgo,
            summary: hoursAgo < 24 ? "groomed today" : hoursAgo < 48 ? "groomed yesterday" : "older grooming"));
    }
}
