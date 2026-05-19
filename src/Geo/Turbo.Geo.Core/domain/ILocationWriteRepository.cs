using Turboapi.Geo.data.model;
using Turboapi.Geo.domain.value;

namespace Turboapi.Geo.domain.query.model;

public interface ILocationWriteRepository
{
    Task<LocationEntity?> GetById(Guid id);
    Task Add(LocationEntity entity);
    Task Update(LocationEntity entity);

    /// <summary>
    /// Applies a partial update to the read-model row, bumping the
    /// <c>UpdatedAt</c> and <c>Version</c> sync fields atomically.
    /// </summary>
    Task UpdatePartial(Guid id, Coordinates? coordinates, DisplayUpdate? display, DateTime updatedAt);

    /// <summary>
    /// Soft-deletes the row: sets <c>DeletedAt</c>, bumps <c>UpdatedAt</c>
    /// and <c>Version</c>. The row stays in the table so delta-sync clients
    /// can learn about the deletion on their next pull.
    /// </summary>
    Task SoftDelete(Guid id, DateTime deletedAt);
}
