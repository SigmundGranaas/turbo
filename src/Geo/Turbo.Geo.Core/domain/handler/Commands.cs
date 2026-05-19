using Turboapi.Geo.domain.value;

namespace Turboapi.Geo.domain.commands
{
    public record CreateLocationCommand
    {
        public Guid UserId { get; init; }
        public Coordinates Coordinates { get; init; }
        public DisplayInformation Display { get; init; }

        public CreateLocationCommand(
            Guid userId,
            Coordinates coordinates,
            DisplayInformation display)
        {
            UserId = userId;
            Coordinates = coordinates;
            Display = display;
        }
    }

    public record UpdateLocationCommand
    {
        public Guid UserId { get; init; }
        public Guid LocationId { get; init; }
        public LocationUpdateParameters Updates { get; init; }

        /// <summary>
        /// Optional optimistic-concurrency token. When non-null the
        /// handler refuses the update unless the row's current
        /// <c>Version</c> matches.
        /// </summary>
        public long? IfMatchVersion { get; init; }

        public UpdateLocationCommand(
            Guid userId,
            Guid locationId,
            LocationUpdateParameters updates,
            long? ifMatchVersion = null)
        {
            UserId = userId;
            LocationId = locationId;
            Updates = updates ?? throw new ArgumentNullException(nameof(updates));
            IfMatchVersion = ifMatchVersion;

            if (!updates.HasAnyChange)
                throw new ArgumentException("At least one update parameter must be specified within the updates.", nameof(updates));
        }
    }

    public record DeleteLocationCommand
    {
        public Guid UserId { get; init; }
        public Guid LocationId { get; init; }

        public long? IfMatchVersion { get; init; }

        public DeleteLocationCommand(Guid userId, Guid locationId, long? ifMatchVersion = null)
        {
            UserId = userId;
            LocationId = locationId;
            IfMatchVersion = ifMatchVersion;
        }
    }
}
