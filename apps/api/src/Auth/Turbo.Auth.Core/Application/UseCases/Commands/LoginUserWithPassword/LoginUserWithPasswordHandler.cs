using Microsoft.Extensions.Logging;
using Turboapi.Auth.Application.Contracts.V1.Auth;
using Turboapi.Auth.Application.Interfaces;
using Turboapi.Auth.Application.Results;
using Turboapi.Auth.Application.Results.Errors;
using Turboapi.Auth.Domain.Aggregates;
using Turboapi.Auth.Domain.Interfaces;

namespace Turboapi.Auth.Application.UseCases.Commands.LoginUserWithPassword
{
    public class LoginUserWithPasswordCommandHandler : ICommandHandler<LoginUserWithPasswordCommand, Result<AuthTokenResponse, LoginError>>
    {
        private readonly IAccountRepository _accountRepository;
        private readonly IPasswordHasher _passwordHasher;
        private readonly IAuthTokenService _authTokenService;
        private readonly ILogger<LoginUserWithPasswordCommandHandler> _logger;

        public LoginUserWithPasswordCommandHandler(
            IAccountRepository accountRepository,
            IPasswordHasher passwordHasher,
            IAuthTokenService authTokenService,
            ILogger<LoginUserWithPasswordCommandHandler> logger)
        {
            _accountRepository = accountRepository;
            _passwordHasher = passwordHasher;
            _authTokenService = authTokenService;
            _logger = logger;
        }

        public async Task<Result<AuthTokenResponse, LoginError>> Handle(
            LoginUserWithPasswordCommand command,
            CancellationToken cancellationToken)
        {
            var account = await _accountRepository.GetByEmailAsync(command.Email);
            if (account == null) return LoginError.AccountNotFound;

            var passwordAuthMethod = account.AuthenticationMethods.OfType<PasswordAuthMethod>().FirstOrDefault();
            if (passwordAuthMethod == null) return LoginError.PasswordMethodNotFound;

            if (!_passwordHasher.VerifyPassword(command.Password, passwordAuthMethod.PasswordHash))
            {
                return LoginError.InvalidCredentials;
            }

            account.UpdateLastLogin();
            account.RecordLoggedIn(passwordAuthMethod.Id, passwordAuthMethod.ProviderName);
            passwordAuthMethod.UpdateLastUsed();

            var newTokens = await _authTokenService.GenerateNewTokenStringsAsync(account);
            account.AddNewRefreshToken(newTokens.RefreshTokenValue, newTokens.RefreshTokenExpiresAt);

            await _accountRepository.UpdateAsync(account);

            // Domain events are drained from the aggregate into the transactional
            // outbox by UnitOfWork.SaveChangesAsync.

            return new AuthTokenResponse(
                newTokens.AccessToken,
                newTokens.RefreshTokenValue,
                account.Id,
                account.Email
            );
        }
    }
}