using FluentAssertions;
using Microsoft.Extensions.DependencyInjection;
using Turbo.Messaging.Nats;
using Turboapi.Activities.BackcountrySki;
using Turboapi.Activities.BackcountrySki.events;
using Turboapi.Activities.events;
using Turboapi.Activities.Fishing;
using Turboapi.Activities.Fishing.events;
using Turboapi.Activities.Freediving;
using Turboapi.Activities.Freediving.events;
using Turboapi.Activities.Hiking;
using Turboapi.Activities.Hiking.events;
using Turboapi.Activities.Packrafting;
using Turboapi.Activities.Packrafting.events;
using Turboapi.Activities.XcSki;
using Turboapi.Activities.XcSki.events;
using Xunit;

namespace Turbo.Activities.Unit;

/// <summary>
/// Each activity kind exposes an <c>Add{Kind}ActivityNatsSubscribers</c>
/// extension that registers five JetStream subscriptions (three typed
/// kind events plus the two cross-kind summary events published off the
/// kind's outbox). These tests don't boot NATS — they assert the
/// registration shape (subject, durable name, event type) so a typo or
/// a missed event type can't slip past CI even when no microservices
/// host is wired up.
///
/// The modulith host uses the in-process equivalent and is covered
/// separately by <c>SubscriberCoverage</c> in the architecture suite.
/// </summary>
public sealed class PerKindNatsSubscriberWiringTests
{
    [Fact]
    public void Fishing_registers_three_kind_events_and_two_summary_events()
    {
        var registrations = RegisterAndCollect(s => s.AddFishingActivityNatsSubscribers());
        AssertKindRegistrationShape(
            registrations,
            kindSubject: "fishing",
            durablePrefix: "fishing-activity",
            summaryDurablePrefix: "fishing-summary",
            createdType: typeof(FishingActivityCreated),
            updatedType: typeof(FishingActivityUpdated),
            deletedType: typeof(FishingActivityDeleted));
    }

    [Fact]
    public void BackcountrySki_registers_with_kind_subject_backcountry_ski()
    {
        var registrations = RegisterAndCollect(s => s.AddBackcountrySkiActivityNatsSubscribers());
        AssertKindRegistrationShape(
            registrations,
            kindSubject: "backcountry_ski",
            durablePrefix: "backcountry-ski-activity",
            summaryDurablePrefix: "backcountry-ski-summary",
            createdType: typeof(BackcountrySkiActivityCreated),
            updatedType: typeof(BackcountrySkiActivityUpdated),
            deletedType: typeof(BackcountrySkiActivityDeleted));
    }

    [Fact]
    public void Hiking_registers_with_kind_subject_hiking()
    {
        var registrations = RegisterAndCollect(s => s.AddHikingActivityNatsSubscribers());
        AssertKindRegistrationShape(
            registrations,
            kindSubject: "hiking",
            durablePrefix: "hiking-activity",
            summaryDurablePrefix: "hiking-summary",
            createdType: typeof(HikingActivityCreated),
            updatedType: typeof(HikingActivityUpdated),
            deletedType: typeof(HikingActivityDeleted));
    }

    [Fact]
    public void XcSki_registers_with_kind_subject_xc_ski()
    {
        var registrations = RegisterAndCollect(s => s.AddXcSkiActivityNatsSubscribers());
        AssertKindRegistrationShape(
            registrations,
            kindSubject: "xc_ski",
            durablePrefix: "xc-ski-activity",
            summaryDurablePrefix: "xc-ski-summary",
            createdType: typeof(XcSkiActivityCreated),
            updatedType: typeof(XcSkiActivityUpdated),
            deletedType: typeof(XcSkiActivityDeleted));
    }

    [Fact]
    public void Packrafting_registers_with_kind_subject_packrafting()
    {
        var registrations = RegisterAndCollect(s => s.AddPackraftingActivityNatsSubscribers());
        AssertKindRegistrationShape(
            registrations,
            kindSubject: "packrafting",
            durablePrefix: "packrafting-activity",
            summaryDurablePrefix: "packrafting-summary",
            createdType: typeof(PackraftingActivityCreated),
            updatedType: typeof(PackraftingActivityUpdated),
            deletedType: typeof(PackraftingActivityDeleted));
    }

    [Fact]
    public void Freediving_registers_with_kind_subject_freediving()
    {
        var registrations = RegisterAndCollect(s => s.AddFreedivingActivityNatsSubscribers());
        AssertKindRegistrationShape(
            registrations,
            kindSubject: "freediving",
            durablePrefix: "freediving-activity",
            summaryDurablePrefix: "freediving-summary",
            createdType: typeof(FreedivingActivityCreated),
            updatedType: typeof(FreedivingActivityUpdated),
            deletedType: typeof(FreedivingActivityDeleted));
    }

    [Fact]
    public void All_six_kinds_use_unique_durable_names_when_composed_in_one_host()
    {
        // Mirrors a hypothetical activities-microservices host that wires
        // every kind side by side. JetStream durable names must be globally
        // unique — a collision would silently merge two subscribers onto
        // the same consumer.
        var services = new ServiceCollection();
        services.AddFishingActivityNatsSubscribers();
        services.AddBackcountrySkiActivityNatsSubscribers();
        services.AddHikingActivityNatsSubscribers();
        services.AddXcSkiActivityNatsSubscribers();
        services.AddPackraftingActivityNatsSubscribers();
        services.AddFreedivingActivityNatsSubscribers();
        var sp = services.BuildServiceProvider();
        var regs = sp.GetServices<NatsSubscriberRegistration>().ToList();

        regs.Should().HaveCount(30, "six kinds × five registrations each");
        regs.Select(r => r.DurableName).Should().OnlyHaveUniqueItems();
        regs.Select(r => r.Subject).Should().OnlyHaveUniqueItems();
    }

    private static IReadOnlyList<NatsSubscriberRegistration> RegisterAndCollect(
        Action<IServiceCollection> register)
    {
        var services = new ServiceCollection();
        register(services);
        return services.BuildServiceProvider()
            .GetServices<NatsSubscriberRegistration>()
            .ToList();
    }

    private static void AssertKindRegistrationShape(
        IReadOnlyList<NatsSubscriberRegistration> registrations,
        string kindSubject,
        string durablePrefix,
        string summaryDurablePrefix,
        Type createdType,
        Type updatedType,
        Type deletedType)
    {
        registrations.Should().HaveCount(5,
            "three typed kind events + two summary events");

        var byType = registrations.ToDictionary(r => r.EventType);

        // Three kind events.
        byType[createdType].Subject.Should().Be(
            $"turbo.activities.{kindSubject}.{createdType.Name}");
        byType[createdType].DurableName.Should().Be($"{durablePrefix}-created");

        byType[updatedType].Subject.Should().Be(
            $"turbo.activities.{kindSubject}.{updatedType.Name}");
        byType[updatedType].DurableName.Should().Be($"{durablePrefix}-updated");

        byType[deletedType].Subject.Should().Be(
            $"turbo.activities.{kindSubject}.{deletedType.Name}");
        byType[deletedType].DurableName.Should().Be($"{durablePrefix}-deleted");

        // Summary events share a type across kinds — each kind binds them on
        // its own subject prefix and durable name so multiple kinds can
        // co-exist in one host.
        var summaryUpserts = registrations
            .Where(r => r.EventType == typeof(ActivitySummaryUpserted))
            .ToList();
        summaryUpserts.Should().HaveCount(1);
        summaryUpserts.Single().Subject.Should().Be(
            $"turbo.activities.{kindSubject}.{nameof(ActivitySummaryUpserted)}");
        summaryUpserts.Single().DurableName.Should().Be($"{summaryDurablePrefix}-upserted");

        var summaryDeletes = registrations
            .Where(r => r.EventType == typeof(ActivitySummaryDeleted))
            .ToList();
        summaryDeletes.Should().HaveCount(1);
        summaryDeletes.Single().Subject.Should().Be(
            $"turbo.activities.{kindSubject}.{nameof(ActivitySummaryDeleted)}");
        summaryDeletes.Single().DurableName.Should().Be($"{summaryDurablePrefix}-deleted");
    }
}
