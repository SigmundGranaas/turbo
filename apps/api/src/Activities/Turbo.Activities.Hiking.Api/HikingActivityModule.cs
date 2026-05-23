using Microsoft.EntityFrameworkCore;
using Turbo.Hosting.Postgres;
using Turbo.Messaging;
using Turbo.Messaging.Nats;
using Turbo.Outbox;
using Turbo.Outbox.Postgres;
using Turboapi.Activities.domain.services;
using Turboapi.Activities.events;
using Turboapi.Activities.Hiking.conditions;
using Turboapi.Activities.Hiking.controller;
using Turboapi.Activities.Hiking.data;
using Turboapi.Activities.Hiking.domain.handler;
using Turboapi.Activities.Hiking.events;
using Turboapi.Activities.value;

namespace Turboapi.Activities.Hiking;

public static class HikingActivityModule
{
    /// <summary>
    /// Postgres schema this module owns. The host never needs to know
    /// this — <see cref="MigrateHikingActivityModuleAsync"/> ensures the
    /// schema exists and runs migrations into it.
    /// </summary>
    private const string Schema = "hiking";

    public static IServiceCollection AddHikingActivityModule(
        this IServiceCollection services,
        string connectionString)
    {
        services.AddDbContext<HikingContext>((sp, options) =>
            options.UseNpgsql(connectionString, npgsql =>
            {
                npgsql.UseNetTopologySuite();
                npgsql.EnableRetryOnFailure();
                npgsql.MigrationsHistoryTable("__EFMigrationsHistory", Schema);
            }));

        services.AddScoped<IHikingActivityReader, EfHikingActivityReader>();
        services.AddScoped<CreateHikingActivityHandler>();
        services.AddScoped<UpdateHikingActivityHandler>();
        services.AddScoped<DeleteHikingActivityHandler>();

        services.AddScoped<HikingActivityCreatedHandler>();
        services.AddScoped<HikingActivityUpdatedHandler>();
        services.AddScoped<HikingActivityDeletedHandler>();
        services.AddScoped<IEventHandler<HikingActivityCreated>>(sp => sp.GetRequiredService<HikingActivityCreatedHandler>());
        services.AddScoped<IEventHandler<HikingActivityUpdated>>(sp => sp.GetRequiredService<HikingActivityUpdatedHandler>());
        services.AddScoped<IEventHandler<HikingActivityDeleted>>(sp => sp.GetRequiredService<HikingActivityDeletedHandler>());

        services.AddScoped<IOutbox<HikingScope>, PgOutbox<HikingContext, HikingScope>>();
        services.AddScoped<IUnitOfWork<HikingScope>, PgUnitOfWork<HikingContext, HikingScope>>();
        services.AddScoped<IIdempotencyStore<HikingContext>, PgIdempotencyStore<HikingContext>>();
        services.AddHostedService<OutboxDispatcherHostedService<HikingContext>>();

        services.AddScoped<IHikingConditionsAdvisor, HikingConditionsAdvisor>();

        services.AddSingleton(new ActivityKindDescriptor
        {
            Key = "hiking",
            DisplayName = "Hiking",
            IconKey = "hiking",
            ColorHex = "#2E7D32",
            AllowedGeometries = new HashSet<ActivityGeometryKind> { ActivityGeometryKind.LineString },
            ConditionsAvailable = true,
        });

        services.AddControllers().AddApplicationPart(typeof(HikingActivitiesController).Assembly);
        return services;
    }

    /// <summary>
    /// Ensures the <c>hiking</c> schema exists and runs all pending EF
    /// Core migrations. Called once at host startup.
    /// </summary>
    public static Task MigrateHikingActivityModuleAsync(
        this IServiceProvider services,
        string connectionString,
        CancellationToken cancellationToken = default)
        => services.MigrateModuleDatabaseAsync<HikingContext>(connectionString, Schema, cancellationToken);

    public static IServiceCollection AddHikingActivityNatsSubscribers(this IServiceCollection services)
    {
        services.AddNatsSubscriber<HikingActivityCreated>(
            "turbo.activities.hiking.HikingActivityCreated", "hiking-activity-created");
        services.AddNatsSubscriber<HikingActivityUpdated>(
            "turbo.activities.hiking.HikingActivityUpdated", "hiking-activity-updated");
        services.AddNatsSubscriber<HikingActivityDeleted>(
            "turbo.activities.hiking.HikingActivityDeleted", "hiking-activity-deleted");
        services.AddNatsSubscriber<ActivitySummaryUpserted>(
            "turbo.activities.hiking.ActivitySummaryUpserted", "hiking-summary-upserted");
        services.AddNatsSubscriber<ActivitySummaryDeleted>(
            "turbo.activities.hiking.ActivitySummaryDeleted", "hiking-summary-deleted");
        return services;
    }
}
