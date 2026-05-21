using Medo;
using NetTopologySuite.Geometries;

namespace Turboapi.Activities.domain;

/// <summary>
/// The identity, ownership, naming, geometry, and version state that every
/// activity carries — regardless of kind. Kept as a value object on
/// purpose: kinds *contain* one of these (composition) rather than
/// inheriting from a common Activity base. The kind aggregate knows when
/// the core needs to mutate (rename, soft-delete, replace geometry, bump
/// version) and calls the corresponding factory.
/// </summary>
public sealed record ActivityCore
{
    public Guid Id { get; init; }
    public Guid OwnerId { get; init; }
    public string Name { get; init; }
    public string? Description { get; init; }
    public Geometry Geometry { get; init; }
    public DateTime CreatedAt { get; init; }
    public DateTime UpdatedAt { get; init; }
    public DateTime? DeletedAt { get; init; }
    public long Version { get; init; }

    private ActivityCore(
        Guid id,
        Guid ownerId,
        string name,
        string? description,
        Geometry geometry,
        DateTime createdAt,
        DateTime updatedAt,
        DateTime? deletedAt,
        long version)
    {
        Id = id;
        OwnerId = ownerId;
        Name = name;
        Description = description;
        Geometry = geometry;
        CreatedAt = createdAt;
        UpdatedAt = updatedAt;
        DeletedAt = deletedAt;
        Version = version;
    }

    public static ActivityCore New(Guid ownerId, string name, string? description, Geometry geometry)
    {
        if (ownerId == Guid.Empty) throw new ArgumentException("Owner id required", nameof(ownerId));
        if (string.IsNullOrWhiteSpace(name)) throw new ArgumentException("Name required", nameof(name));
        ArgumentNullException.ThrowIfNull(geometry);
        var now = DateTime.UtcNow;
        return new ActivityCore(
            id: Uuid7.NewUuid7(),
            ownerId: ownerId,
            name: name,
            description: description,
            geometry: geometry,
            createdAt: now,
            updatedAt: now,
            deletedAt: null,
            version: 1);
    }

    public static ActivityCore Reconstitute(
        Guid id, Guid ownerId, string name, string? description, Geometry geometry,
        DateTime createdAt, DateTime updatedAt, DateTime? deletedAt, long version) =>
        new(id, ownerId, name, description, geometry, createdAt, updatedAt, deletedAt, version);

    public ActivityCore WithRename(string? newName, string? newDescription) =>
        this with
        {
            Name = string.IsNullOrWhiteSpace(newName) ? Name : newName,
            Description = newDescription ?? Description,
            UpdatedAt = DateTime.UtcNow,
            Version = Version + 1,
        };

    public ActivityCore WithGeometry(Geometry newGeometry)
    {
        ArgumentNullException.ThrowIfNull(newGeometry);
        return this with
        {
            Geometry = newGeometry,
            UpdatedAt = DateTime.UtcNow,
            Version = Version + 1,
        };
    }

    public ActivityCore WithSoftDelete()
    {
        var now = DateTime.UtcNow;
        return this with
        {
            DeletedAt = now,
            UpdatedAt = now,
            Version = Version + 1,
        };
    }

    public ActivityCore BumpVersion() => this with
    {
        UpdatedAt = DateTime.UtcNow,
        Version = Version + 1,
    };
}
