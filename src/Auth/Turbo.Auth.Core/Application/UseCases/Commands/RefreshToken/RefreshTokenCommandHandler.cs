using Microsoft.Extensions.Logging;
using Turboapi.Auth.Application.Contracts.V1.Auth;
using Turboapi.Auth.Application.Interfaces;
using Turboapi.Auth.Application.Results;
using Turboapi.Auth.Application.Results.Errors;
using Turboapi.Auth.Domain.Interfaces;

namespace Turboapi.Auth.Application.UseCases.Commands.RefreshToken
{
    public class RefreshTokenCommandHandler : ICommandHandler<RefreshTokenCommand, Result<AuthTokenResponse, RefreshTokenError>>
    {
        private readonly IAccountRepository _accountRepository;
        private readonly IAuthTokenService _authTokenService;
        private readonly ILogger<RefreshTokenCommandHandler> _logger;

        public RefreshTokenCommandHandler(
            IAccountRepository accountRepository,
            IAuthTokenService authTokenService,
            ILogger<RefreshTokenCommandHandler> logger)
        {
            _accountRepository = accountRepository;
            _authTokenService = authTokenService;
            _logger = logger;
        }

        public async Task<Result<AuthTokenResponse, RefreshTokenError>> Handle(
            RefreshTokenCommand command,
            CancellationToken cancellationToken)
        {
            var account = await _accountRepository.GetByRefreshTokenAsync(command.RefreshTokenString);
            
            if (account == null)
            {
                // To prevent token scanning, we don't differentiate between not found, revoked, or expired.
                return RefreshTokenError.InvalidToken;
            }

            var newGeneratedTokenStrings = await _authTokenService.GenerateNewTokenStringsAsync(account);

            var domainRotationResult = account.RotateRefreshToken(
                command.RefreshTokenString,
                newGeneratedTokenStrings.RefreshTokenValue,
                newGeneratedTokenStrings.RefreshTokenExpiresAt
            );

            if (domainRotationResult.IsFailure)
            {
                // This will catch if the token was found but was expired, and handle domain logic errors.
                return domainRotationResult.Error;
            }

            await _accountRepository.UpdateAsync(account);

            // Domain events are drained from the aggregate into the transactional
            // outbox by UnitOfWork.SaveChangesAsync.

            return new AuthTokenResponse(
                newGeneratedTokenStrings.AccessToken,
                newGeneratedTokenStrings.RefreshTokenValue,
                account.Id,
                account.Email
            );
        }
    }
}