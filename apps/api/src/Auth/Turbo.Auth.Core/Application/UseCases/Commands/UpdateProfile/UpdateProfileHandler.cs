using Microsoft.Extensions.Logging;
using Turboapi.Auth.Application.Contracts.V1.Auth;
using Turboapi.Auth.Application.Interfaces;
using Turboapi.Auth.Application.Results;
using Turboapi.Auth.Application.Results.Errors;
using Turboapi.Auth.Domain.Interfaces;

namespace Turboapi.Auth.Application.UseCases.Commands.UpdateProfile
{
    public class UpdateProfileCommandHandler
        : ICommandHandler<UpdateProfileCommand, Result<ProfileResponse, UpdateProfileError>>
    {
        private readonly IAccountRepository _accountRepository;
        private readonly ILogger<UpdateProfileCommandHandler> _logger;

        public UpdateProfileCommandHandler(
            IAccountRepository accountRepository,
            ILogger<UpdateProfileCommandHandler> logger)
        {
            _accountRepository = accountRepository;
            _logger = logger;
        }

        public async Task<Result<ProfileResponse, UpdateProfileError>> Handle(
            UpdateProfileCommand command,
            CancellationToken cancellationToken)
        {
            var account = await _accountRepository.GetByIdAsync(command.AccountId);
            if (account == null)
            {
                _logger.LogWarning("Profile update requested for unknown account {AccountId}", command.AccountId);
                return UpdateProfileError.AccountNotFound;
            }

            var result = account.UpdateProfile(command.DisplayName);
            if (result.IsFailure)
            {
                return result.Error!;
            }

            await _accountRepository.UpdateAsync(account);
            return new ProfileResponse(account.Id, account.Email, account.DisplayName);
        }
    }
}
