namespace Turboapi.Activities.domain.services;

/// <summary>
/// One unit of fan-out work in the orchestrator pipeline. Kinds declare a
/// list of these in their <c>PlanFanOut</c> override; the pipeline runs
/// them in parallel via <c>Task.WhenAll</c> and captures each into a
/// <see cref="ProviderResult"/> — so a single provider failure becomes
/// provenance, not a thrown exception that kills the whole analysis.
/// </summary>
public sealed class ProviderTask
{
    public string Key { get; }
    public Func<CancellationToken, Task<object?>> Run { get; }

    /// <summary>Optional. When set, the pipeline tags the
    /// <see cref="ProviderResult.AgeSeconds"/> based on this timestamp on
    /// the returned slice (e.g. the slice's <c>ValidAt</c> property).
    /// Providers that don't surface a meaningful observed-at time can
    /// leave this null.</summary>
    public Func<object, DateTimeOffset?>? ExtractObservedAt { get; }

    public ProviderTask(
        string key,
        Func<CancellationToken, Task<object?>> run,
        Func<object, DateTimeOffset?>? extractObservedAt = null)
    {
        Key = key ?? throw new ArgumentNullException(nameof(key));
        Run = run ?? throw new ArgumentNullException(nameof(run));
        ExtractObservedAt = extractObservedAt;
    }
}

/// <summary>
/// Outcome of a single <see cref="ProviderTask"/>. Synthesizers read the
/// typed <see cref="Slice"/> via <c>Get&lt;T&gt;(key)</c> on
/// <see cref="SynthesisInput"/>; the pipeline rolls these up into
/// <c>Provenance.Sources</c> for client display.
/// </summary>
public sealed record ProviderResult(
    string Key,
    object? Slice,
    bool Ok,
    bool FromCache,
    DateTimeOffset FetchedAt,
    int? AgeSeconds,
    string? Error);
