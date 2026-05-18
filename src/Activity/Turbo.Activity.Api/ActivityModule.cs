using Microsoft.EntityFrameworkCore;
using Turbo.Messaging;
using Turbo.Messaging.Nats;
using Turbo.Outbox;
using Turbo.Outbox.Postgres;
using Turboapi.Activity.controller;
using Turboapi.Activity.data;
using Turboapi.Activity.domain.events;
using Turboapi.Activity.domain.handler;
using Turboapi.Activity.domain.query;

namespace Turboapi.Activity;

/// <summary>
/// Composition entry point for the Activity module. The extension wires
/// the module's persistence, command/query handlers, projection
/// subscribers, and outbox dispatcher — but not the messaging transport
/// (NATS or in-process), the auth scheme, or the HTTP pipeline. Hosts
/// pick those.
/// </summary>
public static class ActivityModule
{
    public const string ConnectionStringName = "Activity";

    public static IServiceCollection AddActivityModule(
        this IServiceCollection services,
        IConfiguration configuration)
    {
        var connectionString = ResolveConnectionString(configuration);
        services.AddDbContext<ActivityContext>(o =>
            o.UseNpgsql(connectionString, npgsql => npgsql.EnableRetryOnFailure()));

        services.AddScoped<IActivityReadRepository, ActivityReadRepository>();
        services.AddScoped<IActivityWriteRepository, ActivityWriteRepository>();

        services.AddScoped<CreateActivityHandler>();
        services.AddScoped<EditActivityHandler>();
        services.AddScoped<DeleteActivityHandler>();
        services.AddScoped<ActivityQueryHandler>();

        services.AddScoped<IEventHandler<ActivityCreated>, ActivityEventHandler>();
        services.AddScoped<IEventHandler<ActivityUpdated>, ActivityEventHandler>();
        services.AddScoped<IEventHandler<ActivityDeleted>, ActivityEventHandler>();

        services.AddScoped<IOutbox<ActivityScope>, PgOutbox<ActivityContext, ActivityScope>>();
        services.AddScoped<IUnitOfWork<ActivityScope>, PgUnitOfWork<ActivityContext, ActivityScope>>();
        services.AddScoped<IIdempotencyStore<ActivityContext>, PgIdempotencyStore<ActivityContext>>();
        services.AddHostedService<OutboxDispatcherHostedService<ActivityContext>>();

        services.AddControllers().AddApplicationPart(typeof(ActivityController).Assembly);

        return services;
    }

    /// <summary>
    /// Registers Activity's projection subscribers on the NATS subjects
    /// produced by AppendActivityEventsAsync. Used by the microservice host
    /// — the modulith host uses the in-process equivalent.
    /// </summary>
    public static IServiceCollection AddActivityNatsSubscribers(this IServiceCollection services)
    {
        services.AddNatsSubscriber<ActivityCreated>(
            "turbo.activity.ActivityCreated", "activity-created");
        services.AddNatsSubscriber<ActivityUpdated>(
            "turbo.activity.ActivityUpdated", "activity-updated");
        services.AddNatsSubscriber<ActivityDeleted>(
            "turbo.activity.ActivityDeleted", "activity-deleted");
        return services;
    }

    /// <summary>
    /// The event types this module's read-model projection consumes.
    /// Hosts use this to declare in-process subscriptions or NATS bindings
    /// without restating each event name. Position-created is internal —
    /// it does not appear on the read-model projection.
    /// </summary>
    public static IReadOnlyList<(Type EventType, string SubjectShortName)> SubscribedEventTypes { get; } =
    [
        (typeof(ActivityCreated), nameof(ActivityCreated)),
        (typeof(ActivityUpdated), nameof(ActivityUpdated)),
        (typeof(ActivityDeleted), nameof(ActivityDeleted)),
    ];

    private static string ResolveConnectionString(IConfiguration configuration) =>
        configuration.GetConnectionString(ConnectionStringName)
            ?? throw new InvalidOperationException(
                $"ConnectionStrings:{ConnectionStringName} is not configured");
}
