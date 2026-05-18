using Microsoft.Extensions.DependencyInjection;
using Turbo.Messaging.InProcess;
using Turboapi.Geo.domain.events;
using Turboapi.Activity.domain.events;

namespace Turbo.Host.Modulith;

/// <summary>
/// Single source of truth for which event subjects the modulith host
/// dispatches in-process. <c>Program.cs</c> calls this; the
/// <c>SubscriberCoverage</c> architecture test reads the same list to
/// assert every IDomainEvent in the modules has either a registered
/// subscriber here or sits on the explicit audit-only allowlist.
///
/// New event added to a module? Add an AddInProcessSubscriber line
/// here OR mark it audit-only in
/// <see cref="AuditOnlyEvents"/>. CI will tell you when you've
/// forgotten.
/// </summary>
public static class SubscriberWiring
{
    /// <summary>
    /// Event types intentionally published to the outbox (for external
    /// auditors / future consumers) but with no in-process subscriber
    /// inside the modulith. Anything not in this set must have an
    /// AddInProcessSubscriber call below.
    /// </summary>
    public static readonly IReadOnlySet<Type> AuditOnlyEvents = new HashSet<Type>
    {
        // Activity's positional event drives no read-model side-effect on
        // its own; ActivityCreated already carries everything the
        // projection needs.
        typeof(ActivityPositionCreated),

        // Every Auth event is audit-only inside the modulith — Auth has
        // no internal projection subscriber. External consumers attach to
        // turbo.auth.> on the JetStream stream when running as
        // microservices.
        typeof(Turboapi.Auth.Domain.Events.AccountCreatedEvent),
        typeof(Turboapi.Auth.Domain.Events.AccountLoggedInEvent),
        typeof(Turboapi.Auth.Domain.Events.AccountLastLoginUpdatedEvent),
        typeof(Turboapi.Auth.Domain.Events.RoleAddedToAccountEvent),
        typeof(Turboapi.Auth.Domain.Events.PasswordAuthMethodAddedEvent),
        typeof(Turboapi.Auth.Domain.Events.OAuthAuthMethodAddedEvent),
        typeof(Turboapi.Auth.Domain.Events.RefreshTokenGeneratedEvent),
        typeof(Turboapi.Auth.Domain.Events.RefreshTokenRevokedEvent),
        typeof(Turboapi.Auth.Domain.Events.SuspiciousRefreshTokenAttemptEvent),
    };

    public static IServiceCollection AddTurboInProcessSubscribers(this IServiceCollection services)
    {
        services.AddInProcessSubscriber<ActivityCreated>("turbo.activity.ActivityCreated");
        services.AddInProcessSubscriber<ActivityUpdated>("turbo.activity.ActivityUpdated");
        services.AddInProcessSubscriber<ActivityDeleted>("turbo.activity.ActivityDeleted");
        services.AddInProcessSubscriber<LocationCreated>("turbo.geo.LocationCreated");
        services.AddInProcessSubscriber<LocationUpdated>("turbo.geo.LocationUpdated");
        services.AddInProcessSubscriber<LocationDeleted>("turbo.geo.LocationDeleted");
        return services;
    }
}
