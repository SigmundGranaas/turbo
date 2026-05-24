using Microsoft.EntityFrameworkCore;
using Turbo.Hosting.Postgres;
using Turbo.Messaging;
using Turbo.Messaging.Nats;
using Turbo.Outbox;
using Turbo.Outbox.Postgres;
using Turboapi.Activities.domain.services;
using Turboapi.Activities.events;
using Turboapi.Activities.Packrafting.conditions;
using Turboapi.Activities.Packrafting.controller;
using Turboapi.Activities.Packrafting.data;
using Turboapi.Activities.Packrafting.domain.handler;
using Turboapi.Activities.Packrafting.events;
using Turboapi.Activities.value;

namespace Turboapi.Activities.Packrafting;

public static class PackraftingActivityModule
{
    private const string Schema = "packrafting";

    public static IServiceCollection AddPackraftingActivityModule(
        this IServiceCollection services,
        string connectionString)
    {
        services.AddDbContext<PackraftingContext>((sp, options) =>
            options.UseNpgsql(connectionString, npgsql =>
            {
                npgsql.UseNetTopologySuite();
                npgsql.EnableRetryOnFailure();
                npgsql.MigrationsHistoryTable("__EFMigrationsHistory", Schema);
                npgsql.UseQuerySplittingBehavior(QuerySplittingBehavior.SplitQuery);
            }));

        services.AddScoped<IPackraftingActivityReader, EfPackraftingActivityReader>();
        services.AddScoped<CreatePackraftingActivityHandler>();
        services.AddScoped<UpdatePackraftingActivityHandler>();
        services.AddScoped<DeletePackraftingActivityHandler>();

        services.AddScoped<PackraftingActivityCreatedHandler>();
        services.AddScoped<PackraftingActivityUpdatedHandler>();
        services.AddScoped<PackraftingActivityDeletedHandler>();
        services.AddScoped<IEventHandler<PackraftingActivityCreated>>(sp => sp.GetRequiredService<PackraftingActivityCreatedHandler>());
        services.AddScoped<IEventHandler<PackraftingActivityUpdated>>(sp => sp.GetRequiredService<PackraftingActivityUpdatedHandler>());
        services.AddScoped<IEventHandler<PackraftingActivityDeleted>>(sp => sp.GetRequiredService<PackraftingActivityDeletedHandler>());

        services.AddScoped<IOutbox<PackraftingScope>, PgOutbox<PackraftingContext, PackraftingScope>>();
        services.AddScoped<IUnitOfWork<PackraftingScope>, PgUnitOfWork<PackraftingContext, PackraftingScope>>();
        services.AddScoped<IIdempotencyStore<PackraftingContext>, PgIdempotencyStore<PackraftingContext>>();
        services.AddHostedService<OutboxDispatcherHostedService<PackraftingContext>>();

        // Packrafting advisor composes weather + (optional) river flow.
        // IRiverFlowProvider is registered by the shared infrastructure
        // module when a provider is configured; resolve it as optional.
        services.AddScoped<IPackraftingConditionsAdvisor>(sp => new PackraftingConditionsAdvisor(
            sp.GetRequiredService<IWeatherProvider>(),
            sp.GetService<IRiverFlowProvider>()));

        services.AddSingleton(new ActivityKindDescriptor
        {
            Key = "packrafting",
            DisplayName = "Packrafting",
            IconKey = "packrafting",
            ColorHex = "#EF6C00",
            AllowedGeometries = new HashSet<ActivityGeometryKind> { ActivityGeometryKind.LineString },
            ConditionsAvailable = true,
        });

        services.AddControllers().AddApplicationPart(typeof(PackraftingActivitiesController).Assembly);
        return services;
    }

    public static Task MigratePackraftingActivityModuleAsync(
        this IServiceProvider services,
        string connectionString,
        CancellationToken cancellationToken = default)
        => services.MigrateModuleDatabaseAsync<PackraftingContext>(connectionString, Schema, cancellationToken);

    public static IServiceCollection AddPackraftingActivityNatsSubscribers(this IServiceCollection services)
    {
        services.AddNatsSubscriber<PackraftingActivityCreated>(
            "turbo.activities.packrafting.PackraftingActivityCreated", "packrafting-activity-created");
        services.AddNatsSubscriber<PackraftingActivityUpdated>(
            "turbo.activities.packrafting.PackraftingActivityUpdated", "packrafting-activity-updated");
        services.AddNatsSubscriber<PackraftingActivityDeleted>(
            "turbo.activities.packrafting.PackraftingActivityDeleted", "packrafting-activity-deleted");
        services.AddNatsSubscriber<ActivitySummaryUpserted>(
            "turbo.activities.packrafting.ActivitySummaryUpserted", "packrafting-summary-upserted");
        services.AddNatsSubscriber<ActivitySummaryDeleted>(
            "turbo.activities.packrafting.ActivitySummaryDeleted", "packrafting-summary-deleted");
        return services;
    }
}
