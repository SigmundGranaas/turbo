using Microsoft.Extensions.Logging;
using Turboapi.Auth.Application.Interfaces;
using Turboapi.Auth.Application.Results;
using Turboapi.Auth.Application.Results.Errors;
using Turboapi.Auth.Domain.Interfaces;

namespace Turboapi.Auth.Application.UseCases.Commands.RevokeRefreshToken
{
    public class RevokeRefreshTokenCommandHandler : ICommandHandler<RevokeRefreshTokenCommand, Result<RefreshTokenError>>
    {
        private readonly IRefreshTokenRepository _refreshTokenRepository;
        private readonly IAccountRepository _accountRepository;
        private readonly ILogger<RevokeRefreshTokenCommandHandler> _logger;

        public RevokeRefreshTokenCommandHandler(
            IRefreshTokenRepository refreshTokenRepository,
            IAccountRepository accountRepository,
            ILogger<RevokeRefreshTokenCommandHandler> logger)
        {
            _refreshTokenRepository = refreshTokenRepository;
            _accountRepository = accountRepository;
            _logger = logger;
        }

        public async Task<Result<RefreshTokenError>> Handle(RevokeRefreshTokenCommand command, CancellationToken cancellationToken)
        {
            if (string.IsNullOrWhiteSpace(command.RefreshToken))
            {
                return RefreshTokenError.InvalidToken;
            }

            var refreshToken = await _refreshTokenRepository.GetByTokenAsync(command.RefreshToken);
            if (refreshToken == null || refreshToken.IsRevoked)
            {
                // To prevent token scanning, we treat "not found" and "already revoked" the same way.
                // We don't return an error to the client, as the desired state (an invalid token) is achieved.
                _logger.LogInformation("Revocation requested for an invalid, unknown, or already revoked token.");
                return Result.Success<RefreshTokenError>();
            }

            var account = await _accountRepository.GetByIdAsync(refreshToken.AccountId);
            if (account == null)
            {
                // This indicates a data integrity issue but we still proceed to revoke the orphaned token.
                _logger.LogWarning("Refresh token {TokenId} belongs to a non-existent account {AccountId}. Revoking token anyway.",
                    refreshToken.Id, refreshToken.AccountId);
                refreshToken.Revoke("Orphaned token revoked on logout attempt");
                await _refreshTokenRepository.UpdateAsync(refreshToken);
                return Result.Success<RefreshTokenError>();
            }

            account.RevokeRefreshToken(command.RefreshToken, "User initiated logout");

            // The UoW decorator saves changes; UnitOfWork.SaveChangesAsync drains
            // the aggregate's domain events into the transactional outbox in the
            // same database transaction.

            _logger.LogInformation("Successfully revoked refresh token for account {AccountId}", account.Id);
            return Result.Success<RefreshTokenError>();
        }
    }
}