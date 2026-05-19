using Turboapi.Collections.domain.value;

namespace Turboapi.Collections.domain.commands;

public record CreateCollectionCommand
{
    public Guid UserId { get; init; }
    public CollectionMetadata Metadata { get; init; }

    public CreateCollectionCommand(Guid userId, CollectionMetadata metadata)
    {
        UserId = userId;
        Metadata = metadata ?? throw new ArgumentNullException(nameof(metadata));
    }
}

public record UpdateCollectionCommand
{
    public Guid UserId { get; init; }
    public Guid CollectionId { get; init; }
    public CollectionMetadataUpdate Updates { get; init; }
    public long? IfMatchVersion { get; init; }

    public UpdateCollectionCommand(
        Guid userId,
        Guid collectionId,
        CollectionMetadataUpdate updates,
        long? ifMatchVersion = null)
    {
        UserId = userId;
        CollectionId = collectionId;
        Updates = updates ?? throw new ArgumentNullException(nameof(updates));
        IfMatchVersion = ifMatchVersion;
        if (!updates.HasAnyChange)
            throw new ArgumentException(
                "At least one update parameter must be specified within the updates.",
                nameof(updates));
    }
}

public record DeleteCollectionCommand
{
    public Guid UserId { get; init; }
    public Guid CollectionId { get; init; }
    public long? IfMatchVersion { get; init; }

    public DeleteCollectionCommand(Guid userId, Guid collectionId, long? ifMatchVersion = null)
    {
        UserId = userId;
        CollectionId = collectionId;
        IfMatchVersion = ifMatchVersion;
    }
}

public record AddItemToCollectionCommand
{
    public Guid UserId { get; init; }
    public Guid CollectionId { get; init; }
    public CollectionItemRef Item { get; init; }
    public long? IfMatchVersion { get; init; }

    public AddItemToCollectionCommand(
        Guid userId,
        Guid collectionId,
        CollectionItemRef item,
        long? ifMatchVersion = null)
    {
        UserId = userId;
        CollectionId = collectionId;
        Item = item ?? throw new ArgumentNullException(nameof(item));
        IfMatchVersion = ifMatchVersion;
    }
}

public record RemoveItemFromCollectionCommand
{
    public Guid UserId { get; init; }
    public Guid CollectionId { get; init; }
    public CollectionItemRef Item { get; init; }
    public long? IfMatchVersion { get; init; }

    public RemoveItemFromCollectionCommand(
        Guid userId,
        Guid collectionId,
        CollectionItemRef item,
        long? ifMatchVersion = null)
    {
        UserId = userId;
        CollectionId = collectionId;
        Item = item ?? throw new ArgumentNullException(nameof(item));
        IfMatchVersion = ifMatchVersion;
    }
}
