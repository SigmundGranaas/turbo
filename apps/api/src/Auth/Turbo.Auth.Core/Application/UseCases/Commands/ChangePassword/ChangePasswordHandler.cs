using Microsoft.Extensions.Logging;
using Turboapi.Auth.Application.Interfaces;
using Turboapi.Auth.Application.Results;
using Turboapi.Auth.Application.Results.Errors;
using Turboapi.Auth.Domain.Interfaces;

namespace Turboapi.Auth.Application.UseCases.Commands.ChangePassword
{
    public class ChangePasswordCommandHandler
        : ICommandHandler<ChangePasswordCommand, Result<ChangePasswordError>>
    {
        private readonly IAccountRepository _accountRepository;
        private readonly IPasswordHasher _passwordHasher;
        private readonly ILogger<ChangePasswordCommandHandler> _logger;

        public ChangePasswordCommandHandler(
            IAccountRepository accountRepository,
            IPasswordHasher passwordHasher,
            ILogger<ChangePasswordCommandHandler> logger)
        {
            _accountRepository = accountRepository;
            _passwordHasher = passwordHasher;
            _logger = logger;
        }

        public async Task<Result<ChangePasswordError>> Handle(
            ChangePasswordCommand command,
            CancellationToken cancellationToken)
        {
            var account = await _accountRepository.GetByIdAsync(command.AccountId);
            if (account == null)
            {
                _logger.LogWarning("Change-password requested for unknown account {AccountId}", command.AccountId);
                return ChangePasswordError.AccountNotFound;
            }

            var result = account.ChangePassword(command.CurrentPassword, command.NewPassword, _passwordHasher);
            if (result.IsFailure)
            {
                return result;
            }

            await _accountRepository.UpdateAsync(account);
            // UnitOfWorkCommandHandlerDecorator persists and drains domain events.
            return Result.Success<ChangePasswordError>();
        }
    }
}
