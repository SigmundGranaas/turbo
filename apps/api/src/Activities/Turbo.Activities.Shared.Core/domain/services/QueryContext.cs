namespace Turboapi.Activities.domain.services;

/// <summary>
/// The "who is asking, when, with what lookback" envelope every
/// orchestrator run receives. Carries the requesting user's id and skill
/// when known so synthesizers can personalize ("you've never skied above
/// Varsom 3 — this route's exposure exceeds your pattern").
/// </summary>
public sealed record QueryContext
{
    public DateTimeOffset At { get; init; }

    /// <summary>How far back the orchestrator may look at history /
    /// observations. Tuned per call site — analysis defaults to 14d,
    /// recommendations to 7d.</summary>
    public TimeSpan Lookback { get; init; }

    public Guid? RequestingUserId { get; init; }
    public UserSkill? Skill { get; init; }

    /// <summary>Skip the (potentially expensive) suggested-window
    /// computation. The recommendation endpoint sets this <c>false</c>
    /// during fan-out scoring.</summary>
    public bool IncludeWindows { get; init; }

    /// <summary>Skip per-driver forecast bands. Same purpose as
    /// <see cref="IncludeWindows"/> — payload + cost reduction.</summary>
    public bool IncludeForecastBands { get; init; }

    public QueryContext(
        DateTimeOffset at,
        TimeSpan lookback,
        Guid? requestingUserId,
        UserSkill? skill,
        bool includeWindows,
        bool includeForecastBands)
    {
        At = at;
        Lookback = lookback;
        RequestingUserId = requestingUserId;
        Skill = skill;
        IncludeWindows = includeWindows;
        IncludeForecastBands = includeForecastBands;
    }

    public static QueryContext ForAnalysis(DateTimeOffset at, Guid? userId = null, UserSkill? skill = null) =>
        new(at, TimeSpan.FromDays(14), userId, skill, includeWindows: true, includeForecastBands: true);

    public static QueryContext ForQuickScore(DateTimeOffset at) =>
        new(at, TimeSpan.FromDays(7), null, null, includeWindows: false, includeForecastBands: false);
}

public enum UserSkill
{
    Beginner,
    Intermediate,
    Advanced,
    Expert,
}
