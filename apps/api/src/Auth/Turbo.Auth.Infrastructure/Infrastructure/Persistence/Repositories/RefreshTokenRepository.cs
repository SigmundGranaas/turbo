using Microsoft.EntityFrameworkCore;
using Turboapi.Auth.Domain.Aggregates;
using Turboapi.Auth.Domain.Interfaces;

namespace Turboapi.Auth.Infrastructure.Persistence.Repositories
{
    public class RefreshTokenRepository : IRefreshTokenRepository
    {
        private readonly AuthDbContext _context;

        public RefreshTokenRepository(AuthDbContext context)
        {
            _context = context ?? throw new ArgumentNullException(nameof(context));
        }

        public async Task<RefreshToken?> GetByTokenAsync(string token)
        {
            return await _context.RefreshTokens
                .FirstOrDefaultAsync(rt => rt.Token == token);
        }

        public async Task AddAsync(RefreshToken refreshToken)
        {
            if (refreshToken == null) throw new ArgumentNullException(nameof(refreshToken));
            
            await _context.RefreshTokens.AddAsync(refreshToken);
            // SaveChangesAsync handled by Unit of Work or Application Service
        }

        public async Task UpdateAsync(RefreshToken refreshToken)
        {
            if (refreshToken == null) throw new ArgumentNullException(nameof(refreshToken));
            
            _context.RefreshTokens.Update(refreshToken);
            // SaveChangesAsync handled by Unit of Work or Application Service
            await Task.CompletedTask; 
        }

        public async Task<IEnumerable<RefreshToken>> GetActiveTokensForAccountAsync(Guid accountId)
        {
            return await _context.RefreshTokens
                .Where(rt => rt.AccountId == accountId && !rt.IsRevoked && rt.ExpiresAt > DateTime.UtcNow)
                .ToListAsync();
        }
    }
}