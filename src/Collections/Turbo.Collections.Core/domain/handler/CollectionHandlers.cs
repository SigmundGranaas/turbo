using Turbo.Outbox;
using Turboapi.Collections.domain.commands;
using Turboapi.Collections.domain.exception;
using Turboapi.Collections.domain.model;
using Turboapi.Collections.domain.query;
using Turboapi.Collections.domain.value;

namespace Turboapi.Collections.domain.handler;

public class CreateCollectionHandler
{
    private readonly IOutbox<CollectionsScope> _outbox;
    private readonly IUnitOfWork<CollectionsScope> _uow;

    public CreateCollectionHandler(IOutbox<CollectionsScope> outbox, IUnitOfWork<CollectionsScope> uow)
    {
        _outbox = outbox;
        _uow = uow;
    }

    public async Task<Guid> Handle(CreateCollectionCommand command)
    {
        var collection = Collection.Create(command.UserId, command.Metadata);
        await _uow.SaveChangesAsync(ct =>
            _outbox.AppendEventsAsync(collection.Id, collection.Events, ct));
        return collection.Id;
    }
}

public class UpdateCollectionHandler
{
    private readonly ICollectionReadRepository _read;
    private readonly IOutbox<CollectionsScope> _outbox;
    private readonly IUnitOfWork<CollectionsScope> _uow;

    public UpdateCollectionHandler(
        ICollectionReadRepository read,
        IOutbox<CollectionsScope> outbox,
        IUnitOfWork<CollectionsScope> uow)
    {
        _read = read;
        _outbox = outbox;
        _uow = uow;
    }

    public async Task<Collection> Handle(UpdateCollectionCommand command)
    {
        var entity = await _read.GetEntityById(command.CollectionId);
        if (entity is null || entity.DeletedAt is not null)
            throw new CollectionNotFoundException(command.CollectionId.ToString());

        if (command.IfMatchVersion is { } expected && entity.Version != expected)
            throw new OptimisticConcurrencyException(expected, entity.Version);

        var metadata = new CollectionMetadata(
            entity.Name, entity.Description, entity.ColorHex, entity.IconKey,
            entity.SortOrder, entity.SavedFilterJson);
        var items = entity.Items
            .Select(i => new CollectionItemRef(i.ItemType, i.ItemUuid))
            .ToList();

        var collection = Collection.Reconstitute(entity.Id, entity.OwnerId, metadata, items);
        collection.Update(command.UserId, command.Updates);

        await _uow.SaveChangesAsync(ct =>
            _outbox.AppendEventsAsync(collection.Id, collection.Events, ct));
        return collection;
    }
}

public class DeleteCollectionHandler
{
    private readonly ICollectionReadRepository _read;
    private readonly IOutbox<CollectionsScope> _outbox;
    private readonly IUnitOfWork<CollectionsScope> _uow;

    public DeleteCollectionHandler(
        ICollectionReadRepository read,
        IOutbox<CollectionsScope> outbox,
        IUnitOfWork<CollectionsScope> uow)
    {
        _read = read;
        _outbox = outbox;
        _uow = uow;
    }

    public async Task Handle(DeleteCollectionCommand command)
    {
        var entity = await _read.GetEntityById(command.CollectionId);
        if (entity is null || entity.DeletedAt is not null)
            throw new CollectionNotFoundException($"Collection with ID {command.CollectionId} not found");

        if (command.IfMatchVersion is { } expected && entity.Version != expected)
            throw new OptimisticConcurrencyException(expected, entity.Version);

        var metadata = new CollectionMetadata(
            entity.Name, entity.Description, entity.ColorHex, entity.IconKey,
            entity.SortOrder, entity.SavedFilterJson);
        var collection = Collection.Reconstitute(entity.Id, entity.OwnerId, metadata, Enumerable.Empty<CollectionItemRef>());
        collection.Delete(command.UserId);

        await _uow.SaveChangesAsync(ct =>
            _outbox.AppendEventsAsync(collection.Id, collection.Events, ct));
    }
}

public class AddItemToCollectionHandler
{
    private readonly ICollectionReadRepository _read;
    private readonly IOutbox<CollectionsScope> _outbox;
    private readonly IUnitOfWork<CollectionsScope> _uow;

    public AddItemToCollectionHandler(
        ICollectionReadRepository read,
        IOutbox<CollectionsScope> outbox,
        IUnitOfWork<CollectionsScope> uow)
    {
        _read = read;
        _outbox = outbox;
        _uow = uow;
    }

    public async Task Handle(AddItemToCollectionCommand command)
    {
        var entity = await _read.GetEntityById(command.CollectionId);
        if (entity is null || entity.DeletedAt is not null)
            throw new CollectionNotFoundException(command.CollectionId.ToString());

        if (command.IfMatchVersion is { } expected && entity.Version != expected)
            throw new OptimisticConcurrencyException(expected, entity.Version);

        var metadata = new CollectionMetadata(
            entity.Name, entity.Description, entity.ColorHex, entity.IconKey,
            entity.SortOrder, entity.SavedFilterJson);
        var items = entity.Items.Select(i => new CollectionItemRef(i.ItemType, i.ItemUuid)).ToList();
        var collection = Collection.Reconstitute(entity.Id, entity.OwnerId, metadata, items);

        collection.AddItem(command.UserId, command.Item);

        await _uow.SaveChangesAsync(ct =>
            _outbox.AppendEventsAsync(collection.Id, collection.Events, ct));
    }
}

public class RemoveItemFromCollectionHandler
{
    private readonly ICollectionReadRepository _read;
    private readonly IOutbox<CollectionsScope> _outbox;
    private readonly IUnitOfWork<CollectionsScope> _uow;

    public RemoveItemFromCollectionHandler(
        ICollectionReadRepository read,
        IOutbox<CollectionsScope> outbox,
        IUnitOfWork<CollectionsScope> uow)
    {
        _read = read;
        _outbox = outbox;
        _uow = uow;
    }

    public async Task Handle(RemoveItemFromCollectionCommand command)
    {
        var entity = await _read.GetEntityById(command.CollectionId);
        if (entity is null || entity.DeletedAt is not null)
            throw new CollectionNotFoundException(command.CollectionId.ToString());

        if (command.IfMatchVersion is { } expected && entity.Version != expected)
            throw new OptimisticConcurrencyException(expected, entity.Version);

        var metadata = new CollectionMetadata(
            entity.Name, entity.Description, entity.ColorHex, entity.IconKey,
            entity.SortOrder, entity.SavedFilterJson);
        var items = entity.Items.Select(i => new CollectionItemRef(i.ItemType, i.ItemUuid)).ToList();
        var collection = Collection.Reconstitute(entity.Id, entity.OwnerId, metadata, items);

        collection.RemoveItem(command.UserId, command.Item);

        await _uow.SaveChangesAsync(ct =>
            _outbox.AppendEventsAsync(collection.Id, collection.Events, ct));
    }
}
