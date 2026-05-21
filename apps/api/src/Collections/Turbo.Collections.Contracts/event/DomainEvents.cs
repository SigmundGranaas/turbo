using System.Text.Json.Serialization;
using Turbo.Messaging;
using Turboapi.Collections.domain.value;

namespace Turboapi.Collections.domain.events;

public record CollectionCreated : DomainEvent
{
    [JsonPropertyName("collectionId")]
    public Guid CollectionId { get; init; }

    [JsonPropertyName("ownerId")]
    public Guid OwnerId { get; init; }

    [JsonPropertyName("metadata")]
    public CollectionMetadata Metadata { get; init; }

    [JsonConstructor]
    public CollectionCreated(Guid collectionId, Guid ownerId, CollectionMetadata metadata)
    {
        CollectionId = collectionId;
        OwnerId = ownerId;
        Metadata = metadata;
    }
}

public record CollectionUpdated : DomainEvent
{
    [JsonPropertyName("collectionId")]
    public Guid CollectionId { get; init; }

    [JsonPropertyName("ownerId")]
    public Guid OwnerId { get; init; }

    [JsonPropertyName("updates")]
    public CollectionMetadataUpdate Updates { get; init; }

    [JsonConstructor]
    public CollectionUpdated(Guid collectionId, Guid ownerId, CollectionMetadataUpdate updates)
    {
        CollectionId = collectionId;
        OwnerId = ownerId;
        Updates = updates;
    }
}

public record CollectionDeleted : DomainEvent
{
    [JsonPropertyName("collectionId")]
    public Guid CollectionId { get; init; }

    [JsonPropertyName("ownerId")]
    public Guid OwnerId { get; init; }

    [JsonConstructor]
    public CollectionDeleted(Guid collectionId, Guid ownerId)
    {
        CollectionId = collectionId;
        OwnerId = ownerId;
    }
}

public record CollectionItemAdded : DomainEvent
{
    [JsonPropertyName("collectionId")]
    public Guid CollectionId { get; init; }

    [JsonPropertyName("ownerId")]
    public Guid OwnerId { get; init; }

    [JsonPropertyName("item")]
    public CollectionItemRef Item { get; init; }

    [JsonConstructor]
    public CollectionItemAdded(Guid collectionId, Guid ownerId, CollectionItemRef item)
    {
        CollectionId = collectionId;
        OwnerId = ownerId;
        Item = item;
    }
}

public record CollectionItemRemoved : DomainEvent
{
    [JsonPropertyName("collectionId")]
    public Guid CollectionId { get; init; }

    [JsonPropertyName("ownerId")]
    public Guid OwnerId { get; init; }

    [JsonPropertyName("item")]
    public CollectionItemRef Item { get; init; }

    [JsonConstructor]
    public CollectionItemRemoved(Guid collectionId, Guid ownerId, CollectionItemRef item)
    {
        CollectionId = collectionId;
        OwnerId = ownerId;
        Item = item;
    }
}
