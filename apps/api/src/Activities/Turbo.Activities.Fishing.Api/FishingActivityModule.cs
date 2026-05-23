using Microsoft.EntityFrameworkCore;
using Turbo.Hosting.Postgres;
using Turbo.Messaging;
using Turbo.Messaging.Nats;
using Turbo.Outbox;
using Turbo.Outbox.Postgres;
using Turboapi.Activities.domain.services;
using Turboapi.Activities.events;
using Turboapi.Activities.Fishing.conditions;
using Turboapi.Activities.Fishing.controller;
using Turboapi.Activities.Fishing.data;
using Turboapi.Activities.Fishing.domain.handler;
using Turboapi.Activities.Fishing.events;
using Turboapi.Activities.value;

namespace Turboapi.Activities.Fishing;

/// <summary>
/// Composition entry point for the Fishing activity kind. Wires its
/// dedicated EF context (with its own outbox + processed-events tables),
/// command + event handlers, the read-side reader implementation, and
/// contributes its <see cref="ActivityKindDescriptor"/> to the shared
/// kind catalog.
/// </summary>
public static class FishingActivityModule
{
    private const string Schema = "fishing";

    public static IServiceCollection AddFishingActivityModule(
        this IServiceCollection services,
        string connectionString)
    {
        services.AddDbContext<FishingContext>((sp, options) =>
            options.UseNpgsql(connectionString, npgsql =>
            {
                npgsql.UseNetTopologySuite();
                npgsql.EnableRetryOnFailure();
                npgsql.MigrationsHistoryTable("__EFMigrationsHistory", Schema);
            }));

        services.AddScoped<IFishingActivityReader, EfFishingActivityReader>();

        services.AddScoped<CreateFishingActivityHandler>();
        services.AddScoped<UpdateFishingActivityHandler>();
        services.AddScoped<DeleteFishingActivityHandler>();

        services.AddScoped<FishingActivityCreatedHandler>();
        services.AddScoped<FishingActivityUpdatedHandler>();
        services.AddScoped<FishingActivityDeletedHandler>();
        services.AddScoped<IEventHandler<FishingActivityCreated>>(sp =>
            sp.GetRequiredService<FishingActivityCreatedHandler>());
        services.AddScoped<IEventHandler<FishingActivityUpdated>>(sp =>
            sp.GetRequiredService<FishingActivityUpdatedHandler>());
        services.AddScoped<IEventHandler<FishingActivityDeleted>>(sp =>
            sp.GetRequiredService<FishingActivityDeletedHandler>());

        services.AddScoped<IOutbox<FishingScope>, PgOutbox<FishingContext, FishingScope>>();
        services.AddScoped<IUnitOfWork<FishingScope>, PgUnitOfWork<FishingContext, FishingScope>>();
        services.AddScoped<IIdempotencyStore<FishingContext>, PgIdempotencyStore<FishingContext>>();
        services.AddHostedService<OutboxDispatcherHostedService<FishingContext>>();

        // Fishing conditions advisor. Composes the IWeatherProvider that
        // the shared module registered (synthetic by default, met.no when
        // MetNo:UserAgent is configured). Tides + river-flow advisors
        // will compose alongside this in follow-ups.
        services.AddScoped<IFishingConditionsAdvisor, FishingConditionsAdvisor>();

        // Contribute the kind descriptor to the shared catalog. Composition:
        // the catalog discovers kinds via DI rather than a hardcoded enum.
        services.AddSingleton(new ActivityKindDescriptor
        {
            Key = "fishing",
            DisplayName = "Fishing",
            IconKey = "fishing",
            ColorHex = "#1E6FB8",
            AllowedGeometries = new HashSet<ActivityGeometryKind> { ActivityGeometryKind.Point },
            ConditionsAvailable = true,
        });

        services.AddControllers().AddApplicationPart(typeof(FishingActivitiesController).Assembly);

        return services;
    }

    /// <summary>
    /// Registers NATS JetStream subscribers for fishing events. Wire this
    /// from the per-kind microservice host (or from an activities-wide
    /// host) after <c>AddNatsMessaging</c>. The modulith host uses
    /// in-process subscribers instead; see <c>SubscriberWiring</c>.
    /// </summary>
    public static Task MigrateFishingActivityModuleAsync(
        this IServiceProvider services,
        string connectionString,
        CancellationToken cancellationToken = default)
        => services.MigrateModuleDatabaseAsync<FishingContext>(connectionString, Schema, cancellationToken);

    public static IServiceCollection AddFishingActivityNatsSubscribers(this IServiceCollection services)
    {
        services.AddNatsSubscriber<FishingActivityCreated>(
            "turbo.activities.fishing.FishingActivityCreated", "fishing-activity-created");
        services.AddNatsSubscriber<FishingActivityUpdated>(
            "turbo.activities.fishing.FishingActivityUpdated", "fishing-activity-updated");
        services.AddNatsSubscriber<FishingActivityDeleted>(
            "turbo.activities.fishing.FishingActivityDeleted", "fishing-activity-deleted");
        services.AddNatsSubscriber<ActivitySummaryUpserted>(
            "turbo.activities.fishing.ActivitySummaryUpserted", "fishing-summary-upserted");
        services.AddNatsSubscriber<ActivitySummaryDeleted>(
            "turbo.activities.fishing.ActivitySummaryDeleted", "fishing-summary-deleted");
        return services;
    }
}
