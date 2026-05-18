using Turboapi.Geo.data.model;
using Turboapi.Geo.domain.value;

namespace Turboapi.Geo.domain.query.model;

public interface ILocationWriteRepository
{
    Task<LocationEntity?> GetById(Guid id);
    Task Add(LocationEntity entity);
    Task Update(LocationEntity entity);
    Task Delete(LocationEntity entity);
    Task UpdatePartial(Guid id, Coordinates? coordinates, DisplayUpdate? display);
}