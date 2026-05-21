using Turbo.Messaging;

namespace Turboapi.Activities.Fishing;

/// <summary>
/// Module marker for the Fishing kind. Carries its own source name so the
/// outbox + envelope subject identify events as
/// <c>turbo.activities.fishing.*</c>; the cross-kind summaries projector
/// subscribes to that subject in addition to other kinds' equivalents.
/// </summary>
public sealed class FishingScope : IModuleScope
{
    public const string Name = "activities.fishing";
    public static string SourceName => Name;
}
