using Turbo.Messaging;

namespace Turboapi.Activities;

/// <summary>
/// Module marker for the Activities shared commit boundary. Used by the
/// summaries projector's outbox + idempotency wiring. Per-kind sub-modules
/// (e.g. Fishing) declare their own scope so each kind owns an independent
/// outbox/processed-events stream.
/// </summary>
public sealed class ActivitiesScope : IModuleScope
{
    public const string Name = "activities";
    public static string SourceName => Name;
}
