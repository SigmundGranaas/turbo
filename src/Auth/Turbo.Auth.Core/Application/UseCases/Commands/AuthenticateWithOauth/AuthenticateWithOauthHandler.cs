using Turbo.Messaging;
using Turbo.Outbox;
using Turboapi.Auth.Application.Contracts.V1.Auth;
using Turboapi.Auth.Application.Interfaces;
using Turboapi.Auth.Application.Results;
using Turboapi.Auth.Application.Results.Errors;
using Turboapi.Auth.Domain.Aggregates;
using Turboapi.Auth.Domain.Events;
using Microsoft.Extensions.Logging;
using Turboapi.Auth.Domain.Interfaces;

namespace Turboapi.Auth.Application.UseCases.Commands.AuthenticateWithOAuth
{
    public class AuthenticateWithOAuthCommandHandler : ICommandHandler<AuthenticateWithOAuthCommand, Result<AuthTokenResponse, OAuthLoginError>>
    {
        private readonly IEnumerable<IOAuthProviderAdapter> _oauthAdapters;
        private readonly IAccountRepository _accountRepository;
        private readonly IAuthTokenService _authTokenService;
        private readonly ILogger<AuthenticateWithOAuthCommandHandler> _logger;
        private const bool EmailMustBeVerified = true;

        public AuthenticateWithOAuthCommandHandler(
            IEnumerable<IOAuthProviderAdapter> oauthAdapters,
            IAccountRepository accountRepository,
            IAuthTokenService authTokenService,
            ILogger<AuthenticateWithOAuthCommandHandler> logger)
        {
            _oauthAdapters = oauthAdapters;
            _accountRepository = accountRepository;
            _authTokenService = authTokenService;
            _logger = logger;
        }

        public async Task<Result<AuthTokenResponse, OAuthLoginError>> Handle(
            AuthenticateWithOAuthCommand command,
            CancellationToken cancellationToken)
        {
            try
            {
                var adapter = _oauthAdapters.FirstOrDefault(a => a.ProviderName.Equals(command.ProviderName, StringComparison.OrdinalIgnoreCase));
                if (adapter == null) return OAuthLoginError.UnsupportedProvider;

                var tokenExchangeResult = await adapter.ExchangeCodeForTokensAsync(command.AuthorizationCode, command.RedirectUri);
                if (tokenExchangeResult.IsFailure) return OAuthLoginError.ProviderError;
                
                var userInfoResult = await adapter.GetUserInfoAsync(tokenExchangeResult.Value!.AccessToken);
                if (userInfoResult.IsFailure) return OAuthLoginError.ProviderError;

                var userInfo = userInfoResult.Value!;
                if (EmailMustBeVerified && !userInfo.IsEmailVerified) return OAuthLoginError.EmailNotVerified;

                var account = await _accountRepository.GetByOAuthAsync(command.ProviderName, userInfo.ExternalId) ?? await _accountRepository.GetByEmailAsync(userInfo.Email);
                
                bool isNewAccount = false;
                if (account == null)
                {
                    isNewAccount = true;
                    account = Account.Create(Guid.NewGuid(), userInfo.Email, new[] { "User" });
                    account.AddOAuthAuthMethod(command.ProviderName, userInfo.ExternalId, tokenExchangeResult.Value!.AccessToken, tokenExchangeResult.Value!.RefreshToken, null);
                }
                else
                {
                    var oauthMethod = account.AuthenticationMethods.OfType<OAuthAuthMethod>()
                        .FirstOrDefault(m => m.ProviderName.Equals(command.ProviderName, StringComparison.OrdinalIgnoreCase));
                    
                    if (oauthMethod == null)
                    {
                        account.AddOAuthAuthMethod(command.ProviderName, userInfo.ExternalId, tokenExchangeResult.Value!.AccessToken, tokenExchangeResult.Value!.RefreshToken, null);
                    }
                    else
                    {
                        oauthMethod.UpdateTokens(tokenExchangeResult.Value!.AccessToken, tokenExchangeResult.Value!.RefreshToken, null);
                    }
                }
                
                account.UpdateLastLogin();
                var currentOAuthMethod = account.AuthenticationMethods.OfType<OAuthAuthMethod>().First(m => m.ProviderName.Equals(command.ProviderName, StringComparison.OrdinalIgnoreCase));
                currentOAuthMethod.UpdateLastUsed();

                var newTokens = await _authTokenService.GenerateNewTokenStringsAsync(account);
                account.AddNewRefreshToken(newTokens.RefreshTokenValue, newTokens.RefreshTokenExpiresAt);

                if (isNewAccount)
                {
                    await _accountRepository.AddAsync(account);
                }
                else
                {
                    await _accountRepository.UpdateAsync(account);
                }

                // Aggregate emits AccountLoggedInEvent through RecordLoggedIn;
                // UnitOfWork drains it into the outbox alongside the other
                // domain events in the same transaction.
                account.RecordLoggedIn(currentOAuthMethod.Id, command.ProviderName);

                return new AuthTokenResponse(newTokens.AccessToken, newTokens.RefreshTokenValue, account.Id, account.Email);
            }
            catch (Exception ex)
            {
                _logger.LogError(ex, "An unexpected error occurred during OAuth authentication for provider {Provider}", command.ProviderName);
                return OAuthLoginError.AccountCreationFailed;
            }
        }
    }
}