
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

    public record GetLocationsChangedSinceQuery(Guid Owner, DateTime Since, int Limit = 500);

    public record LocationData(
        Guid id,
        Guid ownerId,
        Coordinates geometry,
        DisplayInformation displayInformation,
        DateTime? createdAt = null,
        DateTime? updatedAt = null,
        DateTime? deletedAt = null,
        long? version = null);

    public record LocationTombstoneData(Guid Id, DateTime DeletedAt, long Version);

    public record LocationDeltaResult(
        IReadOnlyList<LocationData> Items,
        IReadOnlyList<LocationTombstoneData> Deleted,
        DateTime ServerTime);
}
