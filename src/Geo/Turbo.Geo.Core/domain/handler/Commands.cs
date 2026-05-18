using Turboapi.Geo.domain.value;

namespace Turboapi.Geo.domain.commands
{
    /// <summary>
    /// Command to create a new location
    /// </summary>
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
        
        public UpdateLocationCommand(
            Guid userId,
            Guid locationId,
            LocationUpdateParameters updates)
        {
            UserId = userId;
            LocationId = locationId;
            Updates = updates ?? throw new ArgumentNullException(nameof(updates));

            if (!updates.HasAnyChange)
                throw new ArgumentException("At least one update parameter must be specified within the updates.", nameof(updates));
        }
    }
    
    /// <summary>
    /// Command to delete a location
    /// </summary>
    public record DeleteLocationCommand
    {
        public Guid UserId { get; init; }
        public Guid LocationId { get; init; }
        
        public DeleteLocationCommand(Guid userId, Guid locationId)
        {
            UserId = userId;
            LocationId = locationId;
        }
    }
}