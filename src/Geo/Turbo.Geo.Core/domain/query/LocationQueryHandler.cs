using Turboapi.Geo.domain.queries;
using System.Collections.Generic;
using System.Linq;
using System.Threading.Tasks;
using System;
using Turboapi.Geo.data.model;
using Turboapi.Geo.domain.model;
using Turboapi.Geo.domain.value;

namespace Turboapi.Geo.domain.query;

public class GetLocationByIdHandler
{
    private readonly ILocationReadRepository _read;

    public GetLocationByIdHandler(ILocationReadRepository read)
    {
        _read = read;
    }

    public async Task<LocationData?> Handle(GetLocationByIdQuery query)
    {
        var locationRead = await _read.GetById(query.LocationId);
        if (locationRead == null)
        {
            return null;
        }
        
        return new LocationData(locationRead.Id, locationRead.OwnerId, locationRead.Coordinates, locationRead.Display);
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
        
        // Apply spatial filtering to spread out locations
        var filteredLocations = FilterOverlappingLocations(locations);
        
        return filteredLocations.Select(loc => 
            new LocationData(loc.Id, loc.OwnerId, loc.Coordinates, loc.Display));
    }
    
    private IEnumerable<Location> FilterOverlappingLocations(IEnumerable<Location> locations)
    {
        // If fewer than max, no need to filter
        var locationsList = locations.ToList();
        if (locationsList.Count <= MaxLocationsToReturn)
        {
            return locationsList;
        }
        
        // Calculate the extent dimensions for adaptive spacing
        var extentWidth = locationsList.Max(l => l.Coordinates.Longitude) - 
                          locationsList.Min(l => l.Coordinates.Longitude);
        var extentHeight = locationsList.Max(l => l.Coordinates.Latitude) - 
                           locationsList.Min(l => l.Coordinates.Latitude);
        
        // Adaptive threshold based on extent size and number of locations
        var adaptiveThreshold = Math.Max(
            MinDistanceThreshold,
            Math.Sqrt((extentWidth * extentHeight) / MaxLocationsToReturn) * 0.2);
        
        var result = new List<Location>();
        var added = new HashSet<Guid>(); 
        
        var sortedLocations = locationsList
            .OrderByDescending(l => l.Display.Name)
            .ThenBy(l => Guid.NewGuid()); // Add randomness as tie-breaker
        
        foreach (var location in sortedLocations)
        {
            // Skip if we've reached the maximum
            if (result.Count >= MaxLocationsToReturn)
                break;
                
            // Skip if this location is too close to any already selected location
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