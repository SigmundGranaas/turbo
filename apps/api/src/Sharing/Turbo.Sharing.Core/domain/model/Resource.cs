using Medo;
using Turboapi.Sharing.value;

namespace Turboapi.Sharing.domain.model;

/// <summary>
/// The universal shareable envelope. Every shareable domain object (a
/// Collection, a Marker, a SavedPath, ...) is paired with exactly one
/// Resource by id. Ownership, visibility, and version live here; payload
/// modules carry only their own fields.
///
/// Sharing operates on Resource. It has no compile-time dependency on any
/// payload type; the <see cref="Type"/> field is just a label.
/// </summary>
public class Resource
{
    public Guid Id { get; private set; }
    public string Type { get; private set; } = string.Empty;
    public Guid OwnerId { get; private set; }
    public Visibility Visibility { get; private set; } = Visibility.Private;
    public long Version { get; private set; }
    public DateTime CreatedAt { get; private set; }
    public DateTime UpdatedAt { get; private set; }
    public DateTime? DeletedAt { get; private set; }

    private Resource() { }

    public static Resource Create(string type, Guid ownerId, Visibility visibility = Visibility.Private)
        => CreateWithId(Uuid7.NewUuid7().ToGuid(), type, ownerId, visibility);

    /// <summary>
    /// Used by payload modules that mint their own UUID first and want the
    /// Resource keyed on that same id. Phase-1 backfill also uses this.
    /// </summary>
    public static Resource CreateWithId(
        Guid id,
        string type,
        Guid ownerId,
        Visibility visibility = Visibility.Private)
    {
        if (string.IsNullOrWhiteSpace(type))
            throw new ArgumentException("Resource type must not be empty", nameof(type));
        var now = DateTime.UtcNow;
        return new Resource
        {
            Id = id,
            Type = type,
            OwnerId = ownerId,
            Visibility = visibility,
            Version = 1,
            CreatedAt = now,
            UpdatedAt = now,
        };
    }

    public void BumpVersion()
    {
        Version += 1;
        UpdatedAt = DateTime.UtcNow;
    }

    public void ChangeVisibility(Visibility next)
    {
        if (next == Visibility) return;
        Visibility = next;
        BumpVersion();
    }

    public void SoftDelete()
    {
        if (DeletedAt is not null) return;
        DeletedAt = DateTime.UtcNow;
        BumpVersion();
    }

    public static Resource Reconstitute(
        Guid id, string type, Guid ownerId, Visibility visibility,
        long version, DateTime createdAt, DateTime updatedAt, DateTime? deletedAt)
        => new()
        {
            Id = id,
            Type = type,
            OwnerId = ownerId,
            Visibility = visibility,
            Version = version,
            CreatedAt = createdAt,
            UpdatedAt = updatedAt,
            DeletedAt = deletedAt,
        };
}
