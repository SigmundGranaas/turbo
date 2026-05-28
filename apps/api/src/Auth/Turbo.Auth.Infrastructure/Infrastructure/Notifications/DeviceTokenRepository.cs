using Microsoft.EntityFrameworkCore;
using Turboapi.Auth.Domain.Interfaces;
using Turboapi.Auth.Domain.Notifications;
using Turboapi.Auth.Infrastructure.Persistence;

namespace Turboapi.Auth.Infrastructure.Notifications
{
    /// <summary>
    /// Device tokens are not an aggregate and aren't part of the outbox/UoW
    /// command path, so this repository persists its own changes directly.
    /// </summary>
    public class DeviceTokenRepository : IDeviceTokenRepository
    {
        private readonly AuthDbContext _context;

        public DeviceTokenRepository(AuthDbContext context)
        {
            _context = context ?? throw new ArgumentNullException(nameof(context));
        }

        public async Task RegisterAsync(Guid accountId, string token, string platform, CancellationToken cancellationToken = default)
        {
            var existing = await _context.DeviceTokens
                .FirstOrDefaultAsync(d => d.Token == token, cancellationToken);

            if (existing == null)
            {
                await _context.DeviceTokens.AddAsync(new DeviceToken(token, accountId, platform), cancellationToken);
            }
            else
            {
                existing.Refresh(accountId, platform);
            }

            await _context.SaveChangesAsync(cancellationToken);
        }

        public async Task RemoveAsync(string token, CancellationToken cancellationToken = default)
        {
            var existing = await _context.DeviceTokens
                .FirstOrDefaultAsync(d => d.Token == token, cancellationToken);
            if (existing == null) return;

            _context.DeviceTokens.Remove(existing);
            await _context.SaveChangesAsync(cancellationToken);
        }

        public async Task<IReadOnlyList<DeviceToken>> GetByAccountAsync(Guid accountId, CancellationToken cancellationToken = default)
        {
            return await _context.DeviceTokens
                .Where(d => d.AccountId == accountId)
                .ToListAsync(cancellationToken);
        }
    }
}
