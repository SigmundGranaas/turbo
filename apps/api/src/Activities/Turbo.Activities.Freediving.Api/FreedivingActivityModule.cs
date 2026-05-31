using Microsoft.EntityFrameworkCore;
using Turbo.Hosting.Postgres;
using Turbo.Messaging;
using Turbo.Messaging.Nats;
using Turbo.Outbox;
using Turbo.Outbox.Postgres;
using Turboapi.Activities.domain.services;
using Turboapi.Activities.events;
using Turboapi.Activities.Freediving.conditions;
using Turboapi.Activities.Freediving.controller;
using Turboapi.Activities.Freediving.data;
using Turboapi.Activities.Freediving.domain.handler;
using Turboapi.Activities.Freediving.events;
using Turboapi.Activities.value;

namespace Turboapi.Activities.Freediving;

public static class FreedivingActivityModule
{
    private const string Schema = "freediving";

    public static IServiceCollection AddFreedivingActivityModule(
        this IServiceCollection services,
        string connectionString)
    {
        services.AddDbContext<FreedivingContext>((sp, options) =>
            options.UseNpgsql(connectionString, npgsql =>
            {
                npgsql.UseNetTopologySuite();
                npgsql.EnableRetryOnFailure();
                npgsql.MigrationsHistoryTable("__EFMigrationsHistory", Schema);
                npgsql.UseQuerySplittingBehavior(QuerySplittingBehavior.SplitQuery);
            }));

        services.AddScoped<IFreedivingActivityReader, EfFreedivingActivityReader>();
        services.AddScoped<CreateFreedivingActivityHandler>();
        services.AddScoped<UpdateFreedivingActivityHandler>();
        services.AddScoped<DeleteFreedivingActivityHandler>();

        services.AddScoped<FreedivingActivityCreatedHandler>();
        services.AddScoped<FreedivingActivityUpdatedHandler>();
        services.AddScoped<FreedivingActivityDeletedHandler>();
        services.AddScoped<IEventHandler<FreedivingActivityCreated>>(sp => sp.GetRequiredService<FreedivingActivityCreatedHandler>());
        services.AddScoped<IEventHandler<FreedivingActivityUpdated>>(sp => sp.GetRequiredService<FreedivingActivityUpdatedHandler>());
        services.AddScoped<IEventHandler<FreedivingActivityDeleted>>(sp => sp.GetRequiredService<FreedivingActivityDeletedHandler>());

        services.AddScoped<IOutbox<FreedivingScope>, PgOutbox<FreedivingContext, FreedivingScope>>();
        services.AddScoped<IUnitOfWork<FreedivingScope>, PgUnitOfWork<FreedivingContext, FreedivingScope>>();
        services.AddScoped<IIdempotencyStore<FreedivingContext>, PgIdempotencyStore<FreedivingContext>>();
        services.AddHostedService<OutboxDispatcherHostedService<FreedivingContext>>();

        // Freediving advisor composes weather + (optional) tides.
        services.AddScoped<IFreedivingConditionsAdvisor>(sp => new FreedivingConditionsAdvisor(
            sp.GetRequiredService<IWeatherProvider>(),
            sp.GetService<ITideProvider>()));

        // v2 orchestrator. Replaces the user-entered visibility field
        // with a computed viz estimate (season × runoff × wind), tide
        // phase driver, HAB / runoff warnings.
        services.AddScoped<FreedivingOrchestrator>();
        services.AddScoped<IActivityRecommendationScorer, FreedivingRecommendationScorer>();

        services.AddSingleton(new ActivityKindDescriptor
        {
            Key = "freediving",
            DisplayName = "Freediving",
            IconKey = "freediving",
            ColorHex = "#1565C0",
            AllowedGeometries = new HashSet<ActivityGeometryKind> { ActivityGeometryKind.Point },
            ConditionsAvailable = true,
        });

        services.AddControllers().AddApplicationPart(typeof(FreedivingActivitiesController).Assembly);
        return services;
    }

    public static Task MigrateFreedivingActivityModuleAsync(
        this IServiceProvider services,
        string connectionString,
        CancellationToken cancellationToken = default)
        => services.MigrateModuleDatabaseAsync<FreedivingContext>(connectionString, Schema, cancellationToken);

    public static IServiceCollection AddFreedivingActivityNatsSubscribers(this IServiceCollection services)
    {
        services.AddNatsSubscriber<FreedivingActivityCreated>(
            "turbo.activities.freediving.FreedivingActivityCreated", "freediving-activity-created");
        services.AddNatsSubscriber<FreedivingActivityUpdated>(
            "turbo.activities.freediving.FreedivingActivityUpdated", "freediving-activity-updated");
        services.AddNatsSubscriber<FreedivingActivityDeleted>(
            "turbo.activities.freediving.FreedivingActivityDeleted", "freediving-activity-deleted");
        services.AddNatsSubscriber<ActivitySummaryUpserted>(
            "turbo.activities.freediving.ActivitySummaryUpserted", "freediving-summary-upserted");
        services.AddNatsSubscriber<ActivitySummaryDeleted>(
            "turbo.activities.freediving.ActivitySummaryDeleted", "freediving-summary-deleted");
        return services;
    }
}
