using Turboapi.Geo.data.model;
using Turboapi.Geo.domain.queries;
using Turboapi.Geo.domain.model;
using Turboapi.Geo.domain.value;
using Turboapi.Sharing;

namespace Turboapi.Geo.domain.query;

public class GetLocationByIdHandler
{
    private readonly ILocationReadRepository _read;
    private readonly IAccessControl _access;

    public GetLocationByIdHandler(ILocationReadRepository read, IAccessControl access)
    {
        _read = read;
        _access = access;
    }

    public async Task<LocationData?> Handle(GetLocationByIdQuery query)
    {
        var entity = await _read.GetEntityById(query.LocationId);
        if (entity is null || entity.DeletedAt is not null) return null;
        // Delegate to the universal sharing gate so a friend with a viewer
        // or editor grant can read this marker even though they don't own it.
        if (!await _access.CanReadAsync(query.Owner, query.LocationId)) return null;

        return new LocationData(
            entity.Id,
            entity.OwnerId,
            Coordinates.FromPoint(entity.Geometry),
            new DisplayInformation(entity.Name, entity.Description, entity.Icon),
            entity.CreatedAt,
            entity.UpdatedAt,
            entity.DeletedAt,
            entity.Version);
    }
}

public class GetLocationsInExtentHandler
{
    private readonly ILocationReadRepository _read;
    private const int MaxLocationsToReturn = 50;
    private const double MinDistanceThreshold = 0.001;

    public GetLocationsInExtentHandler(ILocationReadRepository read)
    {
        _read = read;
    }

    public async Task<IEnumerable<LocationData>> Handle(GetLocationsInExtentQuery query)
    {
        var locations = await _read.GetLocationsInExtent(
            query.Owner,
            query.MinLongitude,
            query.MinLatitude,
            query.MaxLongitude,
            query.MaxLatitude
        );

        var filteredLocations = FilterOverlappingLocations(locations);

        return filteredLocations.Select(loc =>
            new LocationData(loc.Id, loc.OwnerId, loc.Coordinates, loc.Display));
    }

    private IEnumerable<Location> FilterOverlappingLocations(IEnumerable<Location> locations)
    {
        var locationsList = locations.ToList();
        if (locationsList.Count <= MaxLocationsToReturn)
        {
            return locationsList;
        }

        var extentWidth = locationsList.Max(l => l.Coordinates.Longitude) -
                          locationsList.Min(l => l.Coordinates.Longitude);
        var extentHeight = locationsList.Max(l => l.Coordinates.Latitude) -
                           locationsList.Min(l => l.Coordinates.Latitude);

        var adaptiveThreshold = Math.Max(
            MinDistanceThreshold,
            Math.Sqrt((extentWidth * extentHeight) / MaxLocationsToReturn) * 0.2);

        var result = new List<Location>();
        var added = new HashSet<Guid>();

        var sortedLocations = locationsList
            .OrderByDescending(l => l.Display.Name)
            .ThenBy(l => Guid.NewGuid());

        foreach (var location in sortedLocations)
        {
            if (result.Count >= MaxLocationsToReturn)
                break;

            bool isTooClose = result.Any(selected =>
                CalculateDistance(selected.Coordinates, location.Coordinates) < adaptiveThreshold);

            if (!isTooClose && !added.Contains(location.Id))
            {
                result.Add(location);
                added.Add(location.Id);
            }
        }

        return result;
    }

    private double CalculateDistance(Coordinates a, Coordinates b)
    {
        return Math.Sqrt(
            Math.Pow(a.Longitude - b.Longitude, 2) +
            Math.Pow(a.Latitude - b.Latitude, 2));
    }
}

public class GetLocationsChangedSinceHandler
{
    private readonly ILocationReadRepository _read;

    public GetLocationsChangedSinceHandler(ILocationReadRepository read)
    {
        _read = read;
    }

    public async Task<LocationDeltaResult> Handle(GetLocationsChangedSinceQuery query)
    {
        var rows = (await _read.GetChangedSince(query.Owner, query.Since, query.Limit)).ToList();

        var items = rows
            .Where(r => r.DeletedAt is null)
            .Select(r => new LocationData(
                r.Id,
                r.OwnerId,
                Coordinates.FromPoint(r.Geometry),
                new DisplayInformation(r.Name, r.Description, r.Icon),
                r.CreatedAt,
                r.UpdatedAt,
                r.DeletedAt,
                r.Version))
            .ToList();

        var deleted = rows
            .Where(r => r.DeletedAt is not null)
            .Select(r => new LocationTombstoneData(r.Id, r.DeletedAt!.Value, r.Version))
            .ToList();

        var serverTime = await _read.GetCurrentServerTime() ?? DateTime.UtcNow;
        return new LocationDeltaResult(items, deleted, serverTime);
    }
}
