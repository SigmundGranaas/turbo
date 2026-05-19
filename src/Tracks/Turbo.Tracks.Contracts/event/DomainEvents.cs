using System.Text.Json.Serialization;
using Turbo.Messaging;
using Turboapi.Tracks.domain.value;

namespace Turboapi.Tracks.domain.events;

/// <summary>Track created event.</summary>
public record TrackCreated : DomainEvent
{
    [JsonPropertyName("trackId")]
    public Guid TrackId { get; init; }

    [JsonPropertyName("ownerId")]
    public Guid OwnerId { get; init; }

    [JsonPropertyName("metadata")]
    public TrackMetadata Metadata { get; init; }

    [JsonPropertyName("geometry")]
    public TrackGeometry Geometry { get; init; }

    [JsonPropertyName("stats")]
    public TrackStats Stats { get; init; }

    [JsonConstructor]
    public TrackCreated(
        Guid trackId,
        Guid ownerId,
        TrackMetadata metadata,
        TrackGeometry geometry,
        TrackStats stats)
    {
        TrackId = trackId;
        OwnerId = ownerId;
        Metadata = metadata;
        Geometry = geometry;
        Stats = stats;
    }
}

/// <summary>Track updated event — carries the proposed change-set.</summary>
public record TrackUpdated : DomainEvent
{
    [JsonPropertyName("trackId")]
    public Guid TrackId { get; init; }

    [JsonPropertyName("ownerId")]
    public Guid OwnerId { get; init; }

    [JsonPropertyName("updates")]
    public TrackUpdateParameters Updates { get; init; }

    [JsonConstructor]
    public TrackUpdated(Guid trackId, Guid ownerId, TrackUpdateParameters updates)
    {
        TrackId = trackId;
        OwnerId = ownerId;
        Updates = updates;
    }
}

/// <summary>Track deleted event — projection marks the read-model row with a tombstone.</summary>
public record TrackDeleted : DomainEvent
{
    [JsonPropertyName("trackId")]
    public Guid TrackId { get; init; }

    [JsonPropertyName("ownerId")]
    public Guid OwnerId { get; init; }

    [JsonConstructor]
    public TrackDeleted(Guid trackId, Guid ownerId)
    {
        TrackId = trackId;
        OwnerId = ownerId;
    }
}
