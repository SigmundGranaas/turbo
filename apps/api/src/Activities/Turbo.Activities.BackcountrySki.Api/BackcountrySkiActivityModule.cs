using Microsoft.EntityFrameworkCore;
using Turbo.Hosting.Postgres;
using Turbo.Messaging;
using Turbo.Messaging.Nats;
using Turbo.Outbox;
using Turbo.Outbox.Postgres;
using Turboapi.Activities.BackcountrySki.conditions;
using Turboapi.Activities.BackcountrySki.controller;
using Turboapi.Activities.BackcountrySki.data;
using Turboapi.Activities.BackcountrySki.domain.handler;
using Turboapi.Activities.BackcountrySki.events;
using Turboapi.Activities.domain.services;
using Turboapi.Activities.events;
using Turboapi.Activities.value;

namespace Turboapi.Activities.BackcountrySki;

/// <summary>
/// Composition entry point for the Backcountry Ski activity kind. Same
/// shape as the Fishing module: dedicated EF context, command + event
/// handlers, read-side reader, outbox dispatcher, and a single
/// <see cref="ActivityKindDescriptor"/> contributed to the shared catalog.
/// </summary>
public static class BackcountrySkiActivityModule
{
    private const string Schema = "backcountry_ski";

    public static IServiceCollection AddBackcountrySkiActivityModule(
        this IServiceCollection services,
        string connectionString)
    {
        services.AddDbContext<BackcountrySkiContext>((sp, options) =>
            options.UseNpgsql(connectionString, npgsql =>
            {
                npgsql.UseNetTopologySuite();
                npgsql.EnableRetryOnFailure();
                npgsql.MigrationsHistoryTable("__EFMigrationsHistory", Schema);
                npgsql.UseQuerySplittingBehavior(QuerySplittingBehavior.SplitQuery);
            }));

        services.AddScoped<IBackcountrySkiActivityReader, EfBackcountrySkiActivityReader>();

        services.AddScoped<CreateBackcountrySkiActivityHandler>();
        services.AddScoped<UpdateBackcountrySkiActivityHandler>();
        services.AddScoped<DeleteBackcountrySkiActivityHandler>();

        services.AddScoped<BackcountrySkiActivityCreatedHandler>();
        services.AddScoped<BackcountrySkiActivityUpdatedHandler>();
        services.AddScoped<BackcountrySkiActivityDeletedHandler>();
        services.AddScoped<IEventHandler<BackcountrySkiActivityCreated>>(sp =>
            sp.GetRequiredService<BackcountrySkiActivityCreatedHandler>());
        services.AddScoped<IEventHandler<BackcountrySkiActivityUpdated>>(sp =>
            sp.GetRequiredService<BackcountrySkiActivityUpdatedHandler>());
        services.AddScoped<IEventHandler<BackcountrySkiActivityDeleted>>(sp =>
            sp.GetRequiredService<BackcountrySkiActivityDeletedHandler>());

        services.AddScoped<IOutbox<BackcountrySkiScope>, PgOutbox<BackcountrySkiContext, BackcountrySkiScope>>();
        services.AddScoped<IUnitOfWork<BackcountrySkiScope>, PgUnitOfWork<BackcountrySkiContext, BackcountrySkiScope>>();
        services.AddScoped<IIdempotencyStore<BackcountrySkiContext>, PgIdempotencyStore<BackcountrySkiContext>>();
        services.AddHostedService<OutboxDispatcherHostedService<BackcountrySkiContext>>();

        // Backcountry-ski conditions advisor. Composes weather +
        // (optional) avalanche provider — synthetic by default, Varsom
        // when configured.
        services.AddScoped<IBackcountrySkiConditionsAdvisor>(sp => new BackcountrySkiConditionsAdvisor(
            sp.GetRequiredService<IWeatherProvider>(),
            sp.GetService<IAvalancheProvider>()));

        // v2 orchestrator. Fans out weather + Varsom + regObs + seNorge
        // in parallel; synthesis emits the structured ActivityAnalysis.
        services.AddScoped<BackcountrySkiOrchestrator>();
        services.AddScoped<IActivityRecommendationScorer, BackcountrySkiRecommendationScorer>();

        services.AddSingleton(new ActivityKindDescriptor
        {
            Key = "backcountry_ski",
            DisplayName = "Backcountry skiing",
            IconKey = "backcountry_ski",
            ColorHex = "#7A3CCB",
            AllowedGeometries = new HashSet<ActivityGeometryKind> { ActivityGeometryKind.LineString },
            ConditionsAvailable = true,
        });

        services.AddControllers().AddApplicationPart(typeof(BackcountrySkiActivitiesController).Assembly);

        return services;
    }

    /// <summary>
    /// NATS JetStream subscribers for backcountry-ski events. Same shape
    /// as <c>AddFishingActivityNatsSubscribers</c>; the modulith host
    /// uses the in-process equivalent.
    /// </summary>
    public static Task MigrateBackcountrySkiActivityModuleAsync(
        this IServiceProvider services,
        string connectionString,
        CancellationToken cancellationToken = default)
        => services.MigrateModuleDatabaseAsync<BackcountrySkiContext>(connectionString, Schema, cancellationToken);

    public static IServiceCollection AddBackcountrySkiActivityNatsSubscribers(this IServiceCollection services)
    {
        services.AddNatsSubscriber<BackcountrySkiActivityCreated>(
            "turbo.activities.backcountry_ski.BackcountrySkiActivityCreated", "backcountry-ski-activity-created");
        services.AddNatsSubscriber<BackcountrySkiActivityUpdated>(
            "turbo.activities.backcountry_ski.BackcountrySkiActivityUpdated", "backcountry-ski-activity-updated");
        services.AddNatsSubscriber<BackcountrySkiActivityDeleted>(
            "turbo.activities.backcountry_ski.BackcountrySkiActivityDeleted", "backcountry-ski-activity-deleted");
        services.AddNatsSubscriber<ActivitySummaryUpserted>(
            "turbo.activities.backcountry_ski.ActivitySummaryUpserted", "backcountry-ski-summary-upserted");
        services.AddNatsSubscriber<ActivitySummaryDeleted>(
            "turbo.activities.backcountry_ski.ActivitySummaryDeleted", "backcountry-ski-summary-deleted");
        return services;
    }
}
