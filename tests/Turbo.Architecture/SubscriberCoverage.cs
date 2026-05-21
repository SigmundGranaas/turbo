using System.Reflection;
using FluentAssertions;
using Microsoft.Extensions.DependencyInjection;
using Turbo.Host.Modulith;
using Turbo.Messaging;
using Turbo.Messaging.InProcess;
using Xunit;

namespace Turbo.Architecture;

/// <summary>
/// A new <see cref="IDomainEvent"/> added to any module must either get a
/// corresponding subscriber registered in <see cref="SubscriberWiring"/>
/// or be opted in to the audit-only allowlist. Otherwise the event is
/// published to the outbox, dispatched to the bus, and silently dropped
/// because no subscriber binds the subject — a regression you wouldn't
/// notice until someone asked why the read model isn't catching the new
/// event.
/// </summary>
public sealed class SubscriberCoverage
{
    [Fact]
    public void every_module_event_either_has_an_in_process_subscriber_or_is_explicitly_audit_only()
    {
        var services = new ServiceCollection();
        services.AddSingleton<Microsoft.Extensions.Logging.ILoggerFactory, Microsoft.Extensions.Logging.LoggerFactory>();
        services.AddInProcessMessaging();
        services.AddTurboInProcessSubscribers();

        var registeredSubjects = services
            .Where(d => d.ServiceType == typeof(InProcessSubscriberRegistration))
            .Select(d => ((InProcessSubscriberRegistration)d.ImplementationInstance!).EventType)
            .ToHashSet();

        var allModuleEvents = ModuleEventInventory();

        var uncovered = allModuleEvents
            .Where(t => !registeredSubjects.Contains(t))
            .Where(t => !SubscriberWiring.AuditOnlyEvents.Contains(t))
            .ToList();

        uncovered.Should().BeEmpty(
            "events with no subscriber and no audit-only allowlist entry will be silently dropped by the modulith deploy; "
            + "either add `AddInProcessSubscriber<X>(...)` to SubscriberWiring or add `typeof(X)` to AuditOnlyEvents. "
            + "Uncovered: " + string.Join(", ", uncovered.Select(t => t.FullName)));
    }

    [Fact]
    public void audit_only_allowlist_does_not_contain_events_that_have_a_subscriber()
    {
        var services = new ServiceCollection();
        services.AddSingleton<Microsoft.Extensions.Logging.ILoggerFactory, Microsoft.Extensions.Logging.LoggerFactory>();
        services.AddInProcessMessaging();
        services.AddTurboInProcessSubscribers();

        var registered = services
            .Where(d => d.ServiceType == typeof(InProcessSubscriberRegistration))
            .Select(d => ((InProcessSubscriberRegistration)d.ImplementationInstance!).EventType)
            .ToHashSet();

        var conflicting = SubscriberWiring.AuditOnlyEvents
            .Where(registered.Contains)
            .ToList();

        conflicting.Should().BeEmpty(
            "an event cannot simultaneously be audit-only AND have an in-process subscriber — the allowlist is for events with no subscriber. "
            + "Remove from AuditOnlyEvents: " + string.Join(", ", conflicting.Select(t => t.FullName)));
    }

    [Fact]
    public void audit_only_allowlist_does_not_contain_unknown_event_types()
    {
        var allModuleEvents = ModuleEventInventory();

        var unknown = SubscriberWiring.AuditOnlyEvents
            .Where(t => !allModuleEvents.Contains(t))
            .ToList();

        unknown.Should().BeEmpty(
            "AuditOnlyEvents references types that no module defines as IDomainEvent. "
            + "Stale entries: " + string.Join(", ", unknown.Select(t => t.FullName)));
    }

    private static HashSet<Type> ModuleEventInventory()
    {
        // Touch a known type from each module's assembly so it's loaded.
        _ = typeof(Turboapi.Tracks.domain.events.TrackCreated);
        _ = typeof(Turboapi.Geo.domain.events.LocationCreated);
        _ = typeof(Turboapi.Auth.Domain.Events.AccountCreatedEvent);
        _ = typeof(Turboapi.Collections.domain.events.CollectionCreated);
        _ = typeof(Turboapi.Activities.events.ActivitySummaryUpserted);
        _ = typeof(Turboapi.Activities.Fishing.events.FishingActivityCreated);
        _ = typeof(Turboapi.Activities.BackcountrySki.events.BackcountrySkiActivityCreated);
        _ = typeof(Turboapi.Activities.Hiking.events.HikingActivityCreated);
        _ = typeof(Turboapi.Activities.XcSki.events.XcSkiActivityCreated);
        _ = typeof(Turboapi.Activities.Packrafting.events.PackraftingActivityCreated);
        _ = typeof(Turboapi.Activities.Freediving.events.FreedivingActivityCreated);

        var moduleAssemblies = new[]
        {
            typeof(Turboapi.Tracks.domain.events.TrackCreated).Assembly,
            typeof(Turboapi.Geo.domain.events.LocationCreated).Assembly,
            typeof(Turboapi.Auth.Domain.Events.AccountCreatedEvent).Assembly,
            typeof(Turboapi.Collections.domain.events.CollectionCreated).Assembly,
            typeof(Turboapi.Activities.events.ActivitySummaryUpserted).Assembly,
            typeof(Turboapi.Activities.Fishing.events.FishingActivityCreated).Assembly,
            typeof(Turboapi.Activities.BackcountrySki.events.BackcountrySkiActivityCreated).Assembly,
            typeof(Turboapi.Activities.Hiking.events.HikingActivityCreated).Assembly,
            typeof(Turboapi.Activities.XcSki.events.XcSkiActivityCreated).Assembly,
            typeof(Turboapi.Activities.Packrafting.events.PackraftingActivityCreated).Assembly,
            typeof(Turboapi.Activities.Freediving.events.FreedivingActivityCreated).Assembly,
        };

        return moduleAssemblies
            .SelectMany(a => a.GetTypes())
            .Where(t => !t.IsAbstract && !t.IsInterface)
            .Where(t => typeof(IDomainEvent).IsAssignableFrom(t))
            .ToHashSet();
    }
}
