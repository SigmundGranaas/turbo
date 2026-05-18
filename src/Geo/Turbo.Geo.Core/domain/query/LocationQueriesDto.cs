
using Turboapi.Geo.domain.value;

namespace Turboapi.Geo.domain.queries
{
    public record GetLocationByIdQuery(Guid LocationId, Guid Owner);
    
    public record GetLocationsInExtentQuery(
        Guid Owner,
        double MinLongitude,
        double MinLatitude,
        double MaxLongitude,
        double MaxLatitude
        ); 
    
    public record LocationData(Guid id, Guid ownerId, Coordinates geometry, DisplayInformation displayInformation);
}