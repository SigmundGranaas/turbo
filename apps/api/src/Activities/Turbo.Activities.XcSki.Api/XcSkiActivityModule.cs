using Microsoft.EntityFrameworkCore;
using Turbo.Hosting.Postgres;
using Turbo.Messaging;
using Turbo.Messaging.Nats;
using Turbo.Outbox;
using Turbo.Outbox.Postgres;
using Turboapi.Activities.domain.services;
using Turboapi.Activities.events;
using Turboapi.Activities.value;
using Turboapi.Activities.XcSki.conditions;
using Turboapi.Activities.XcSki.controller;
using Turboapi.Activities.XcSki.data;
using Turboapi.Activities.XcSki.domain.handler;
using Turboapi.Activities.XcSki.events;

namespace Turboapi.Activities.XcSki;

public static class XcSkiActivityModule
{
    private const string Schema = "xc_ski";

    public static IServiceCollection AddXcSkiActivityModule(
        this IServiceCollection services,
        string connectionString)
    {
        services.AddDbContext<XcSkiContext>((sp, options) =>
            options.UseNpgsql(connectionString, npgsql =>
            {
                npgsql.UseNetTopologySuite();
                npgsql.EnableRetryOnFailure();
                npgsql.MigrationsHistoryTable("__EFMigrationsHistory", Schema);
                npgsql.UseQuerySplittingBehavior(QuerySplittingBehavior.SplitQuery);
            }));

        services.AddScoped<IXcSkiActivityReader, EfXcSkiActivityReader>();
        services.AddScoped<CreateXcSkiActivityHandler>();
        services.AddScoped<UpdateXcSkiActivityHandler>();
        services.AddScoped<DeleteXcSkiActivityHandler>();

        services.AddScoped<XcSkiActivityCreatedHandler>();
        services.AddScoped<XcSkiActivityUpdatedHandler>();
        services.AddScoped<XcSkiActivityDeletedHandler>();
        services.AddScoped<IEventHandler<XcSkiActivityCreated>>(sp => sp.GetRequiredService<XcSkiActivityCreatedHandler>());
        services.AddScoped<IEventHandler<XcSkiActivityUpdated>>(sp => sp.GetRequiredService<XcSkiActivityUpdatedHandler>());
        services.AddScoped<IEventHandler<XcSkiActivityDeleted>>(sp => sp.GetRequiredService<XcSkiActivityDeletedHandler>());

        services.AddScoped<IOutbox<XcSkiScope>, PgOutbox<XcSkiContext, XcSkiScope>>();
        services.AddScoped<IUnitOfWork<XcSkiScope>, PgUnitOfWork<XcSkiContext, XcSkiScope>>();
        services.AddScoped<IIdempotencyStore<XcSkiContext>, PgIdempotencyStore<XcSkiContext>>();
        services.AddHostedService<OutboxDispatcherHostedService<XcSkiContext>>();

        services.AddScoped<IXcSkiConditionsAdvisor, XcSkiConditionsAdvisor>();
        services.AddScoped<XcSkiOrchestrator>();
        services.AddScoped<IActivityRecommendationScorer, XcSkiRecommendationScorer>();

        services.AddSingleton(new ActivityKindDescriptor
        {
            Key = "xc_ski",
            DisplayName = "XC skiing",
            IconKey = "xc_ski",
            ColorHex = "#00838F",
            AllowedGeometries = new HashSet<ActivityGeometryKind> { ActivityGeometryKind.LineString },
            ConditionsAvailable = true,
        });

        services.AddControllers().AddApplicationPart(typeof(XcSkiActivitiesController).Assembly);
        return services;
    }

    public static Task MigrateXcSkiActivityModuleAsync(
        this IServiceProvider services,
        string connectionString,
        CancellationToken cancellationToken = default)
        => services.MigrateModuleDatabaseAsync<XcSkiContext>(connectionString, Schema, cancellationToken);

    public static IServiceCollection AddXcSkiActivityNatsSubscribers(this IServiceCollection services)
    {
        services.AddNatsSubscriber<XcSkiActivityCreated>(
            "turbo.activities.xc_ski.XcSkiActivityCreated", "xc-ski-activity-created");
        services.AddNatsSubscriber<XcSkiActivityUpdated>(
            "turbo.activities.xc_ski.XcSkiActivityUpdated", "xc-ski-activity-updated");
        services.AddNatsSubscriber<XcSkiActivityDeleted>(
            "turbo.activities.xc_ski.XcSkiActivityDeleted", "xc-ski-activity-deleted");
        services.AddNatsSubscriber<ActivitySummaryUpserted>(
            "turbo.activities.xc_ski.ActivitySummaryUpserted", "xc-ski-summary-upserted");
        services.AddNatsSubscriber<ActivitySummaryDeleted>(
            "turbo.activities.xc_ski.ActivitySummaryDeleted", "xc-ski-summary-deleted");
        return services;
    }
}
