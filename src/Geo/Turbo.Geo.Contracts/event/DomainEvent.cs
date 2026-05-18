using System.Text.Json.Serialization;
using Turbo.Messaging;
using Turboapi.Geo.domain.value;

namespace Turboapi.Geo.domain.events
{
    /// <summary>
    /// Location created event
    /// </summary>
    public record LocationCreated : DomainEvent
    {
        [JsonPropertyName("locationId")]
        public Guid LocationId { get; init; }

        [JsonPropertyName("ownerId")]
        public Guid OwnerId { get; init; }

        [JsonPropertyName("coordinates")]
        public Coordinates Coordinates { get; init; }

        [JsonPropertyName("display")]
        public DisplayInformation Display { get; init; }

        [JsonConstructor]
        public LocationCreated(
            Guid locationId,
            Guid ownerId,
            Coordinates coordinates,
            DisplayInformation display)
        {
            LocationId = locationId;
            OwnerId = ownerId;
            Coordinates = coordinates;
            Display = display;
        }
    }

    /// <summary>
    /// Location updated event with changes
    /// </summary>
    public record LocationUpdated : DomainEvent
    {
        [JsonPropertyName("locationId")]
        public Guid LocationId { get; init; }

        [JsonPropertyName("ownerId")]
        public Guid OwnerId { get; init; }

        [JsonPropertyName("updates")]
        public LocationUpdateParameters Updates { get; init; }

        [JsonConstructor]
        public LocationUpdated(
            Guid locationId,
            Guid ownerId,
            LocationUpdateParameters updates)
        {
            LocationId = locationId;
            OwnerId = ownerId;
            Updates = updates;
        }
    }

    /// <summary>
    /// Location deleted event
    /// </summary>
    public record LocationDeleted : DomainEvent
    {
        [JsonPropertyName("locationId")]
        public Guid LocationId { get; init; }

        [JsonPropertyName("ownerId")]
        public Guid OwnerId { get; init; }

        [JsonConstructor]
        public LocationDeleted(Guid locationId, Guid ownerId)
        {
            LocationId = locationId;
            OwnerId = ownerId;
        }
    }
}
