using Medo;
using Turbo.Messaging;
using Turboapi.Geo.domain.events;
using Turboapi.Geo.domain.exception;
using Turboapi.Geo.domain.value;

namespace Turboapi.Geo.domain.model
{
      public class Location
    {
        public Guid Id { get; private set; }
        public Guid OwnerId { get; private set; }
        public Coordinates Coordinates { get; private set; }
        public DisplayInformation Display { get; private set; }

        private readonly List<DomainEvent> _events = new();
        public IReadOnlyList<DomainEvent> Events => _events.AsReadOnly();

        private Location() { }

        public static Location Create(Guid ownerId, Coordinates coordinates, DisplayInformation display)
        {
            var location = new Location
            {
                Id = Uuid7.NewUuid7(),
                OwnerId = ownerId,
                Coordinates = coordinates,
                Display = display ?? throw new ArgumentNullException(nameof(display)) // Ensure display is not null on creation
            };

            location._events.Add(new LocationCreated(
                location.Id,
                location.OwnerId,
                location.Coordinates,
                location.Display));

            return location;
        }

        // Signature now uses domain-specific LocationUpdateParameters
        public void Update(Guid requestUserId, LocationUpdateParameters updates)
        {
            EnsureUserIsAuthorized(requestUserId);

            if (updates == null) // Should be caught by command constructor, but defensive check
                throw new ArgumentNullException(nameof(updates));

            // If no actual changes are proposed by the parameters object, do nothing.
            if (!updates.HasAnyChange)
            {
                return;
            }

            bool anyEffectiveChangeMade = false;
            Coordinates? updatedCoordinatesForEvent = null;
            DisplayInformation? newDisplayInformationForEvent = null;

            // 1. Handle Coordinate Update
            if (updates.Coordinates != null)
            {
                // Check if newCoordinates is actually different from current Coordinates
                // Assuming Coordinates has proper equality implemented.
                if (!Coordinates.Equals(updates.Coordinates))
                {
                    Coordinates = updates.Coordinates;
                    updatedCoordinatesForEvent = Coordinates;
                    anyEffectiveChangeMade = true;
                }
            }

            // 2. Handle Display Information Update
            if (updates.Display != null && updates.Display.HasAnyChange)
            {
                var displayChanges = updates.Display; // This is domain.value.DisplayUpdate

                // Start with current values
                string currentName = this.Display.Name;
                string? currentDescription = this.Display.Description;
                string? currentIcon = this.Display.Icon;
                bool displayPropertyChanged = false;

                // If Name is provided in the changeset (not null) and different, update it
                if (displayChanges.Name != null && displayChanges.Name != currentName)
                {
                    currentName = displayChanges.Name;
                    displayPropertyChanged = true;
                }
                // If Description is provided (not null) and different, update it
                if (displayChanges.Description != null && displayChanges.Description != currentDescription)
                {
                    currentDescription = displayChanges.Description;
                    displayPropertyChanged = true;
                }
                // If Icon is provided (not null) and different, update it
                if (displayChanges.Icon != null && displayChanges.Icon != currentIcon)
                {
                    currentIcon = displayChanges.Icon;
                    displayPropertyChanged = true;
                }

                if (displayPropertyChanged)
                {
                    var newDisplay = new DisplayInformation(currentName, currentDescription, currentIcon);
                    Display = newDisplay;
                    newDisplayInformationForEvent = Display;
                    anyEffectiveChangeMade = true;
                }
            }

            if (anyEffectiveChangeMade)
            {
                // The LocationUpdates record is still appropriate here as it describes
                // the *resulting state* of Coordinates and Display if they changed.
                _events.Add(new LocationUpdated(
                    Id,
                    OwnerId,
                    new LocationUpdateParameters()
                    {
                        Coordinates = updatedCoordinatesForEvent,
                        Display = updates.Display
                    }
                ));
            }
        }

        public void Delete(Guid requestUserId)
        {
            EnsureUserIsAuthorized(requestUserId);
            _events.Add(new LocationDeleted(Id, OwnerId));
        }

        private void EnsureUserIsAuthorized(Guid requestUserId)
        {
            if (OwnerId != requestUserId)
                throw new UnauthorizedException("Only the owner can modify this location");
        }

        public static Location Reconstitute(
            Guid id,
            Guid ownerId,
            Coordinates coordinates,
            DisplayInformation display)
        {
            return new Location
            {
                Id = id,
                OwnerId = ownerId,
                Coordinates = coordinates,
                Display = display
            };
        }
    }
}