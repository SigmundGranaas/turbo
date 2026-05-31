using System.Text.Json;
using System.Text.Json.Serialization;

namespace Turboapi.Activities.value;

/// <summary>
/// The wire shape produced by each kind's orchestrator. Unlike the legacy
/// per-kind <c>{Kind}ConditionsReport</c> types — which collapse everything
/// into a score plus a one-line rationale — an <see cref="ActivityAnalysis"/>
/// surfaces the named drivers behind the score, forecast bands per metric,
/// suggested time windows, warnings, and source provenance so the client can
/// render *why*, not just *what*. Kind-specific structured payloads ride in
/// <see cref="KindSlices"/> so the contract stays extensible without leaking
/// per-kind types into the shared assembly.
/// </summary>
public sealed record ActivityAnalysis
{
    [JsonPropertyName("activityId")] public Guid ActivityId { get; init; }
    [JsonPropertyName("kind")] public string Kind { get; init; }
    [JsonPropertyName("validAt")] public DateTimeOffset ValidAt { get; init; }
    [JsonPropertyName("fetchedAt")] public DateTimeOffset FetchedAt { get; init; }

    /// <summary>0–100 composite. <c>null</c> means the orchestrator could
    /// not produce a defensible score (e.g. every upstream failed). Clients
    /// must render the rationale + warnings regardless.</summary>
    [JsonPropertyName("score")] public int? Score { get; init; }

    [JsonPropertyName("confidence")] public ScoreConfidence Confidence { get; init; }

    /// <summary>One-paragraph human-readable summary that complements the
    /// structured drivers. Localized server-side.</summary>
    [JsonPropertyName("rationale")] public string Rationale { get; init; }

    [JsonPropertyName("drivers")] public IReadOnlyList<Driver> Drivers { get; init; }
    [JsonPropertyName("bands")] public IReadOnlyList<ForecastBand> Bands { get; init; }
    [JsonPropertyName("warnings")] public IReadOnlyList<Warning> Warnings { get; init; }
    [JsonPropertyName("suggestedWindows")] public IReadOnlyList<TimeWindow> SuggestedWindows { get; init; }

    /// <summary>Kind-specific structured payload (e.g.
    /// <c>{"backcountrySki": {avalancheLevel: 3, perAspectLoading: …}}</c>).
    /// Keys are stable kind identifiers (matching <c>ActivityKindCatalog</c>);
    /// values are <see cref="JsonElement"/> so they round-trip without forcing
    /// every kind's type into Shared.Contracts.</summary>
    [JsonPropertyName("kindSlices")] public IReadOnlyDictionary<string, JsonElement> KindSlices { get; init; }

    [JsonPropertyName("provenance")] public Provenance Provenance { get; init; }

    [JsonConstructor]
    public ActivityAnalysis(
        Guid activityId,
        string kind,
        DateTimeOffset validAt,
        DateTimeOffset fetchedAt,
        int? score,
        ScoreConfidence confidence,
        string rationale,
        IReadOnlyList<Driver> drivers,
        IReadOnlyList<ForecastBand> bands,
        IReadOnlyList<Warning> warnings,
        IReadOnlyList<TimeWindow> suggestedWindows,
        IReadOnlyDictionary<string, JsonElement> kindSlices,
        Provenance provenance)
    {
        ActivityId = activityId;
        Kind = kind ?? throw new ArgumentNullException(nameof(kind));
        ValidAt = validAt;
        FetchedAt = fetchedAt;
        Score = score;
        Confidence = confidence;
        Rationale = rationale ?? throw new ArgumentNullException(nameof(rationale));
        Drivers = drivers ?? Array.Empty<Driver>();
        Bands = bands ?? Array.Empty<ForecastBand>();
        Warnings = warnings ?? Array.Empty<Warning>();
        SuggestedWindows = suggestedWindows ?? Array.Empty<TimeWindow>();
        KindSlices = kindSlices ?? new Dictionary<string, JsonElement>();
        Provenance = provenance ?? throw new ArgumentNullException(nameof(provenance));
    }
}

/// <summary>
/// One named contributor to the analysis. Carries a current value, the
/// weight it contributes to <see cref="ActivityAnalysis.Score"/>, the
/// orchestrator's confidence in the value, and an optional forecast band
/// for that metric so the UI can render a per-driver sparkline.
/// </summary>
public sealed record Driver
{
    /// <summary>Stable identifier (e.g. <c>"wind_loading"</c>,
    /// <c>"viz_estimate"</c>, <c>"flow_percentile"</c>). Used as the i18n
    /// key for <see cref="Label"/>; clients may also key icons off it.</summary>
    [JsonPropertyName("key")] public string Key { get; init; }
    [JsonPropertyName("label")] public string Label { get; init; }
    [JsonPropertyName("unit")] public string? Unit { get; init; }
    [JsonPropertyName("value")] public double? Value { get; init; }

    /// <summary>Share of the composite score this driver contributes
    /// (0–1). Should sum to ~1 across all drivers, though drift is fine —
    /// the UI renders this as a relative bar.</summary>
    [JsonPropertyName("weight")] public double Weight { get; init; }

    /// <summary>0–1. Drivers with <c>0</c> confidence are typically
    /// excluded from the composite score by the synthesizer.</summary>
    [JsonPropertyName("confidence")] public double Confidence { get; init; }

    /// <summary>Optional qualitative direction ("rising", "falling",
    /// "stable", "loaded NE"). Free-form string, localized server-side.</summary>
    [JsonPropertyName("direction")] public string? Direction { get; init; }

    [JsonPropertyName("band")] public ForecastBand? Band { get; init; }
    [JsonPropertyName("rationale")] public string? Rationale { get; init; }

    [JsonConstructor]
    public Driver(
        string key, string label, string? unit, double? value,
        double weight, double confidence, string? direction,
        ForecastBand? band, string? rationale)
    {
        Key = key ?? throw new ArgumentNullException(nameof(key));
        Label = label ?? throw new ArgumentNullException(nameof(label));
        Unit = unit;
        Value = value;
        Weight = weight;
        Confidence = confidence;
        Direction = direction;
        Band = band;
        Rationale = rationale;
    }
}

/// <summary>
/// Time-series for a single metric (visibility, flow, wind, …). Each
/// sample carries a value and an optional confidence interval so the
/// client can shade uncertainty.
/// </summary>
public sealed record ForecastBand
{
    [JsonPropertyName("metric")] public string Metric { get; init; }
    [JsonPropertyName("samples")] public IReadOnlyList<ForecastSample> Samples { get; init; }
    [JsonPropertyName("trend")] public string? Trend { get; init; }
    [JsonPropertyName("confidence")] public double Confidence { get; init; }

    [JsonConstructor]
    public ForecastBand(string metric, IReadOnlyList<ForecastSample> samples, string? trend, double confidence)
    {
        Metric = metric ?? throw new ArgumentNullException(nameof(metric));
        Samples = samples ?? Array.Empty<ForecastSample>();
        Trend = trend;
        Confidence = confidence;
    }
}

public sealed record ForecastSample
{
    [JsonPropertyName("at")] public DateTimeOffset At { get; init; }
    [JsonPropertyName("value")] public double Value { get; init; }
    [JsonPropertyName("lower")] public double? Lower { get; init; }
    [JsonPropertyName("upper")] public double? Upper { get; init; }

    [JsonConstructor]
    public ForecastSample(DateTimeOffset at, double value, double? lower, double? upper)
    {
        At = at;
        Value = value;
        Lower = lower;
        Upper = upper;
    }
}

/// <summary>
/// A suggested time interval to do the activity, paired with a qualitative
/// rating + a one-line reason. The orchestrator emits zero or more of
/// these; the UI's primary CTA is usually "go now / go later" based on
/// the first window's <see cref="Quality"/>.
/// </summary>
public sealed record TimeWindow
{
    [JsonPropertyName("start")] public DateTimeOffset Start { get; init; }
    [JsonPropertyName("end")] public DateTimeOffset End { get; init; }
    [JsonPropertyName("quality")] public WindowQuality Quality { get; init; }
    [JsonPropertyName("label")] public string Label { get; init; }
    [JsonPropertyName("reason")] public string? Reason { get; init; }

    [JsonConstructor]
    public TimeWindow(DateTimeOffset start, DateTimeOffset end, WindowQuality quality, string label, string? reason)
    {
        Start = start;
        End = end;
        Quality = quality;
        Label = label ?? throw new ArgumentNullException(nameof(label));
        Reason = reason;
    }
}

public sealed record Warning
{
    /// <summary>Stable code for analytics and i18n
    /// (<c>LEVEL_4_OR_5_AVOID</c>, <c>HAB_ALERT</c>, …).</summary>
    [JsonPropertyName("code")] public string Code { get; init; }
    [JsonPropertyName("severity")] public Severity Severity { get; init; }
    [JsonPropertyName("title")] public string Title { get; init; }
    [JsonPropertyName("body")] public string Body { get; init; }
    [JsonPropertyName("sourceUrl")] public string? SourceUrl { get; init; }

    [JsonConstructor]
    public Warning(string code, Severity severity, string title, string body, string? sourceUrl)
    {
        Code = code ?? throw new ArgumentNullException(nameof(code));
        Severity = severity;
        Title = title ?? throw new ArgumentNullException(nameof(title));
        Body = body ?? throw new ArgumentNullException(nameof(body));
        SourceUrl = sourceUrl;
    }
}

public sealed record Provenance
{
    [JsonPropertyName("sources")] public IReadOnlyList<SourceHit> Sources { get; init; }
    [JsonPropertyName("durationMs")] public long DurationMs { get; init; }

    [JsonConstructor]
    public Provenance(IReadOnlyList<SourceHit> sources, long durationMs)
    {
        Sources = sources ?? Array.Empty<SourceHit>();
        DurationMs = durationMs;
    }
}

public sealed record SourceHit
{
    [JsonPropertyName("providerKey")] public string ProviderKey { get; init; }
    [JsonPropertyName("ok")] public bool Ok { get; init; }
    [JsonPropertyName("fromCache")] public bool FromCache { get; init; }

    /// <summary>Age of the underlying observation in seconds (i.e. how
    /// stale the data the orchestrator used is). <c>null</c> when the
    /// provider couldn't be reached.</summary>
    [JsonPropertyName("ageSeconds")] public int? AgeSeconds { get; init; }

    [JsonPropertyName("error")] public string? Error { get; init; }

    [JsonConstructor]
    public SourceHit(string providerKey, bool ok, bool fromCache, int? ageSeconds, string? error)
    {
        ProviderKey = providerKey ?? throw new ArgumentNullException(nameof(providerKey));
        Ok = ok;
        FromCache = fromCache;
        AgeSeconds = ageSeconds;
        Error = error;
    }
}

[JsonConverter(typeof(JsonStringEnumConverter))]
public enum ScoreConfidence
{
    Low,
    Medium,
    High,
}

[JsonConverter(typeof(JsonStringEnumConverter))]
public enum WindowQuality
{
    Excellent,
    Good,
    Marginal,
    Avoid,
}

[JsonConverter(typeof(JsonStringEnumConverter))]
public enum Severity
{
    Info,
    Caution,
    Danger,
}
