using Turbo.Messaging;

namespace Turboapi.Activities.BackcountrySki;

/// <summary>
/// Module marker for the Backcountry Ski kind. Events live under
/// <c>turbo.activities.backcountry_ski.*</c>; the kind owns its own
/// outbox + idempotency store inside its dedicated database.
/// </summary>
public sealed class BackcountrySkiScope : IModuleScope
{
    public const string Name = "activities.backcountry_ski";
    public static string SourceName => Name;
}
