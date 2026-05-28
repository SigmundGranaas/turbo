using Microsoft.EntityFrameworkCore;
using Microsoft.Extensions.DependencyInjection.Extensions;
using Turbo.Messaging;
using Turbo.Messaging.Nats;
using Turbo.Outbox;
using Turbo.Outbox.Postgres;
using Turboapi.Collections.controller;
using Turboapi.Collections.data;
using Turboapi.Collections.domain.events;
using Turboapi.Collections.domain.handler;
using Turboapi.Collections.domain.query;
using Turboapi.Sharing;
using Turboapi.Sharing.data;

namespace Turboapi.Collections;

public static class CollectionsModule
{
    public const string ConnectionStringName = "Collections";

    public static IServiceCollection AddCollectionsModule(
        this IServiceCollection services,
        IConfiguration configuration)
    {
        var connectionString = ResolveConnectionString(configuration);

        services.AddDbContext<CollectionsReadContext>((sp, options) =>
            options.UseNpgsql(connectionString, npgsql =>
            {
                npgsql.EnableRetryOnFailure();
                npgsql.UseQuerySplittingBehavior(QuerySplittingBehavior.SplitQuery);
            }));

        services.AddScoped<ICollectionWriteRepository, EfCollectionWriteRepository>();
        services.AddScoped<ICollectionReadRepository, EfCollectionWriteRepository.EfCollectionReadRepository>();

        services.AddScoped<CollectionCreatedHandler>();
        services.AddScoped<CollectionUpdatedHandler>();
        services.AddScoped<CollectionDeletedHandler>();
        services.AddScoped<CollectionItemAddedHandler>();
        services.AddScoped<CollectionItemRemovedHandler>();
        services.AddScoped<IEventHandler<CollectionCreated>>(sp => sp.GetRequiredService<CollectionCreatedHandler>());
        services.AddScoped<IEventHandler<CollectionUpdated>>(sp => sp.GetRequiredService<CollectionUpdatedHandler>());
        services.AddScoped<IEventHandler<CollectionDeleted>>(sp => sp.GetRequiredService<CollectionDeletedHandler>());
        services.AddScoped<IEventHandler<CollectionItemAdded>>(sp => sp.GetRequiredService<CollectionItemAddedHandler>());
        services.AddScoped<IEventHandler<CollectionItemRemoved>>(sp => sp.GetRequiredService<CollectionItemRemovedHandler>());

        services.AddScoped<GetCollectionByIdHandler>();
        services.AddScoped<GetUserCollectionsHandler>();
        services.AddScoped<GetCollectionsChangedSinceHandler>();
        services.AddScoped<CreateCollectionHandler>();
        services.AddScoped<UpdateCollectionHandler>();
        services.AddScoped<DeleteCollectionHandler>();
        services.AddScoped<AddItemToCollectionHandler>();
        services.AddScoped<RemoveItemFromCollectionHandler>();

        services.AddScoped<IOutbox<CollectionsScope>, PgOutbox<CollectionsReadContext, CollectionsScope>>();
        services.AddScoped<IUnitOfWork<CollectionsScope>, PgUnitOfWork<CollectionsReadContext, CollectionsScope>>();
        services.AddScoped<IIdempotencyStore<CollectionsReadContext>, PgIdempotencyStore<CollectionsReadContext>>();
        services.AddHostedService<OutboxDispatcherHostedService<CollectionsReadContext>>();

        services.AddControllers().AddApplicationPart(typeof(CollectionsController).Assembly);

        return services;
    }

    /// <summary>
    /// Registers an IAccessControl implementation backed by a read-only
    /// view of the Sharing schema. Required by the standalone Collections
    /// host so its read/write handlers can consult grants. In the modulith
    /// deploy, Sharing's own module already registers IAccessControl, so
    /// this method's TryAdd calls are no-ops.
    /// </summary>
    public static IServiceCollection AddCollectionsAccessControl(
        this IServiceCollection services,
        IConfiguration configuration)
    {
        var sharingConn = configuration.GetConnectionString("Sharing")
            ?? throw new InvalidOperationException(
                "ConnectionStrings:Sharing must be configured so Collections can consult IAccessControl.");

        services.AddDbContext<SharingReadContext>(o =>
            o.UseNpgsql(sharingConn, npgsql => npgsql.EnableRetryOnFailure()));
        services.TryAddScoped<IAccessControl, EfAccessControl>();
        return services;
    }

    public static IServiceCollection AddCollectionsNatsSubscribers(this IServiceCollection services)
    {
        services.AddNatsSubscriber<CollectionCreated>("turbo.collections.CollectionCreated", "collections-collection-created");
        services.AddNatsSubscriber<CollectionUpdated>("turbo.collections.CollectionUpdated", "collections-collection-updated");
        services.AddNatsSubscriber<CollectionDeleted>("turbo.collections.CollectionDeleted", "collections-collection-deleted");
        services.AddNatsSubscriber<CollectionItemAdded>("turbo.collections.CollectionItemAdded", "collections-item-added");
        services.AddNatsSubscriber<CollectionItemRemoved>("turbo.collections.CollectionItemRemoved", "collections-item-removed");
        return services;
    }

    private static string ResolveConnectionString(IConfiguration configuration) =>
        configuration.GetConnectionString(ConnectionStringName)
            ?? throw new InvalidOperationException(
                $"ConnectionStrings:{ConnectionStringName} is not configured");
}
