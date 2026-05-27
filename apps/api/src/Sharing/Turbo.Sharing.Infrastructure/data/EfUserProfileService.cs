using System.Security.Cryptography;
using Microsoft.EntityFrameworkCore;
using Microsoft.Extensions.Logging;
using Npgsql;
using Turboapi.Sharing.data.model;
using Turboapi.Sharing.domain.service;

namespace Turboapi.Sharing.data;

public sealed class EfUserProfileService : IUserProfileService
{
    /// <summary>
    /// Curated alphabet for friend codes: lowercase letters and digits
    /// minus the ambiguous ones (l, 1, i, o, 0). 31 chars; 7 of them
    /// give ~28 bits of entropy, comfortably collision-resistant for
    /// turbo-scale user counts with retries on collision.
    /// </summary>
    private const string Alphabet = "abcdefghjkmnpqrstuvwxyz23456789";

    /// <summary>Length of the random portion of a friend code.</summary>
    private const int CodeLength = 7;

    /// <summary>Max retries on UNIQUE collision before giving up.</summary>
    private const int MaxRetries = 5;

    private readonly SharingReadContext _db;
    private readonly ILogger<EfUserProfileService> _logger;

    public EfUserProfileService(SharingReadContext db, ILogger<EfUserProfileService> logger)
    {
        _db = db;
        _logger = logger;
    }

    public async Task<UserProfileDto> EnsureProfileAsync(Guid userId, CancellationToken cancellationToken = default)
    {
        var existing = await _db.UserProfiles
            .AsNoTracking()
            .FirstOrDefaultAsync(p => p.UserId == userId, cancellationToken);
        if (existing is not null)
            return new UserProfileDto(existing.UserId, existing.FriendCode, existing.CreatedAt);

        for (var attempt = 0; attempt < MaxRetries; attempt++)
        {
            var candidate = GenerateCode();
            try
            {
                var entity = new UserProfileEntity
                {
                    UserId = userId,
                    FriendCode = candidate,
                };
                _db.UserProfiles.Add(entity);
                await _db.SaveChangesAsync(cancellationToken);
                await _db.Entry(entity).ReloadAsync(cancellationToken);
                return new UserProfileDto(entity.UserId, entity.FriendCode, entity.CreatedAt);
            }
            catch (DbUpdateException ex) when (ex.InnerException is PostgresException pg && pg.SqlState == "23505")
            {
                // Either the user already has a profile (race) or the code collided.
                _db.ChangeTracker.Clear();
                var racedExisting = await _db.UserProfiles
                    .AsNoTracking()
                    .FirstOrDefaultAsync(p => p.UserId == userId, cancellationToken);
                if (racedExisting is not null)
                    return new UserProfileDto(racedExisting.UserId, racedExisting.FriendCode, racedExisting.CreatedAt);
                _logger.LogDebug("Friend-code collision on attempt {Attempt} — retrying", attempt + 1);
            }
        }
        throw new InvalidOperationException(
            $"Could not generate a unique friend code after {MaxRetries} attempts.");
    }

    public async Task<Guid?> LookupByCodeAsync(string friendCode, CancellationToken cancellationToken = default)
    {
        if (string.IsNullOrWhiteSpace(friendCode)) return null;
        var normalized = friendCode.Trim().ToLowerInvariant();
        // Strip an optional "turbo-" prefix the client may include.
        if (normalized.StartsWith("turbo-")) normalized = normalized[6..];

        var userId = await _db.UserProfiles
            .AsNoTracking()
            .Where(p => p.FriendCode == normalized)
            .Select(p => (Guid?)p.UserId)
            .FirstOrDefaultAsync(cancellationToken);
        return userId;
    }

    private static string GenerateCode()
    {
        var chars = new char[CodeLength];
        for (var i = 0; i < CodeLength; i++)
        {
            chars[i] = Alphabet[RandomNumberGenerator.GetInt32(Alphabet.Length)];
        }
        return new string(chars);
    }
}
