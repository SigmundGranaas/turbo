using Microsoft.Extensions.Logging;
using Turboapi.Auth.Application.Contracts.V1.Auth;
using Turboapi.Auth.Application.Interfaces; // Implement ICommandHandler
using Turboapi.Auth.Application.Results;
using Turboapi.Auth.Application.Results.Errors;
using Turboapi.Auth.Domain.Aggregates;
using Turboapi.Auth.Domain.Interfaces;

namespace Turboapi.Auth.Application.UseCases.Commands.RegisterUserWithPassword
{
    // Implement the ICommandHandler interface
    public class RegisterUserWithPasswordCommandHandler : ICommandHandler<RegisterUserWithPasswordCommand, Result<AuthTokenResponse, RegistrationError>>
    {
        private readonly IAccountRepository _accountRepository;
        private readonly IPasswordHasher _passwordHasher;
        private readonly IAuthTokenService _authTokenService;
        private readonly ILogger<RegisterUserWithPasswordCommandHandler> _logger;

        public RegisterUserWithPasswordCommandHandler(
            IAccountRepository accountRepository,
            IPasswordHasher passwordHasher,
            IAuthTokenService authTokenService,
            ILogger<RegisterUserWithPasswordCommandHandler> logger)
        {
            _accountRepository = accountRepository;
            _passwordHasher = passwordHasher;
            _authTokenService = authTokenService;
            _logger = logger;
        }

        public async Task<Result<AuthTokenResponse, RegistrationError>> Handle(
            RegisterUserWithPasswordCommand command,
            CancellationToken cancellationToken)
        {
            if (await _accountRepository.GetByEmailAsync(command.Email) != null)
            {
                return RegistrationError.EmailAlreadyExists;
            }

            var account = Account.Create(Guid.NewGuid(), command.Email, new[] { "User" });
            account.AddPasswordAuthMethod(command.Password, _passwordHasher);
            
            var newTokens = await _authTokenService.GenerateNewTokenStringsAsync(account);
            account.AddNewRefreshToken(newTokens.RefreshTokenValue, newTokens.RefreshTokenExpiresAt);
            
            await _accountRepository.AddAsync(account);

            // Domain events are drained from the aggregate into the transactional
            // outbox by UnitOfWork.SaveChangesAsync (invoked by the
            // UnitOfWorkCommandHandlerDecorator after this handler returns).

            return new AuthTokenResponse(
                newTokens.AccessToken,
                newTokens.RefreshTokenValue,
                account.Id,
                account.Email
            );
        }
    }
}