using Turboapi.Geo.domain.model;

namespace Turboapi.Geo.domain.query;

public interface ILocationReadRepository
{
    Task<Location?> GetById(Guid id);
    Task<IEnumerable<Location>> GetLocationsInExtent(
        Guid ownerId,
        double minLongitude,
        double minLatitude,
        double maxLongitude,
        double maxLatitude
    );
}