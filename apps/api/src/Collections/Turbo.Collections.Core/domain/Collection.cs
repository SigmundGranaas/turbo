using Medo;
using Turbo.Messaging;
using Turboapi.Collections.domain.events;
using Turboapi.Collections.domain.exception;
using Turboapi.Collections.domain.value;

namespace Turboapi.Collections.domain.model;

/// <summary>
/// Collection aggregate root: a named group of items (markers, paths,
/// future types) owned by a single user. Items are stored as opaque
/// (type, uuid) pairs — the server never dereferences into the source
/// module to verify the item still exists. Smart collections store a
/// <c>SavedFilterJson</c> blob which the client interprets.
/// </summary>
public class Collection
{
    public Guid Id { get; private set; }
    public Guid OwnerId { get; private set; }
    public CollectionMetadata Metadata { get; private set; } = CollectionMetadata.Empty;

    private readonly HashSet<CollectionItemRef> _items = new();
    public IReadOnlyCollection<CollectionItemRef> Items => _items;

    private readonly List<DomainEvent> _events = new();
    public IReadOnlyList<DomainEvent> Events => _events.AsReadOnly();

    private Collection() { }

    public static Collection Create(Guid ownerId, CollectionMetadata metadata)
    {
        if (metadata is null) throw new ArgumentNullException(nameof(metadata));
        if (string.IsNullOrWhiteSpace(metadata.Name))
            throw new ArgumentException("Collection name must not be empty", nameof(metadata));

        var collection = new Collection
        {
            Id = Uuid7.NewUuid7(),
            OwnerId = ownerId,
            Metadata = metadata,
        };
        collection._events.Add(new CollectionCreated(collection.Id, collection.OwnerId, collection.Metadata));
        return collection;
    }

    public void Update(Guid requestUserId, CollectionMetadataUpdate updates)
    {
        if (updates is null) throw new ArgumentNullException(nameof(updates));
        if (!updates.HasAnyChange) return;

        var name = updates.Name ?? Metadata.Name;
        var description = updates.Description ?? Metadata.Description;
        var colorHex = updates.ColorHex ?? Metadata.ColorHex;
        var iconKey = updates.IconKey ?? Metadata.IconKey;
        var sortOrder = updates.SortOrder ?? Metadata.SortOrder;
        var savedFilter = updates.ClearSavedFilter
            ? null
            : (updates.SavedFilterJson ?? Metadata.SavedFilterJson);

        var next = new CollectionMetadata(name, description, colorHex, iconKey, sortOrder, savedFilter);
        if (next.Equals(Metadata)) return;

        Metadata = next;
        _events.Add(new CollectionUpdated(Id, OwnerId, updates));
    }

    public void AddItem(Guid requestUserId, CollectionItemRef item)
    {
        EnsureItemIsValid(item);

        if (_items.Add(item))
            _events.Add(new CollectionItemAdded(Id, OwnerId, item));
    }

    public void RemoveItem(Guid requestUserId, CollectionItemRef item)
    {
        EnsureItemIsValid(item);

        if (_items.Remove(item))
            _events.Add(new CollectionItemRemoved(Id, OwnerId, item));
    }

    public void Delete(Guid requestUserId)
    {
        _events.Add(new CollectionDeleted(Id, OwnerId));
    }

    // Authorization moved out of the aggregate. Write handlers gate
    // mutations through Turboapi.Sharing.IAccessControl so users with an
    // editor grant on the resource can also mutate, not just the owner.

    private static void EnsureItemIsValid(CollectionItemRef item)
    {
        if (item is null) throw new ArgumentNullException(nameof(item));
        if (string.IsNullOrWhiteSpace(item.Type))
            throw new ArgumentException("Item type must be a non-empty string", nameof(item));
        if (string.IsNullOrWhiteSpace(item.Uuid))
            throw new ArgumentException("Item uuid must be a non-empty string", nameof(item));
    }

    public static Collection Reconstitute(
        Guid id,
        Guid ownerId,
        CollectionMetadata metadata,
        IEnumerable<CollectionItemRef> items)
    {
        var collection = new Collection
        {
            Id = id,
            OwnerId = ownerId,
            Metadata = metadata,
        };
        foreach (var item in items) collection._items.Add(item);
        return collection;
    }
}
