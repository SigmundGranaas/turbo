using System.Security.Claims;
using Microsoft.AspNetCore.Authentication.Cookies;
using Microsoft.AspNetCore.Authentication.JwtBearer;
using Microsoft.AspNetCore.Authorization;
using Microsoft.AspNetCore.Mvc;
using Turboapi.Auth.Application.Contracts.V1.Auth;
using Turboapi.Auth.Application.Interfaces;
using Turboapi.Auth.Application.Results;
using Turboapi.Auth.Application.Results.Errors;
using Turboapi.Auth.Application.UseCases.Commands.UpdateProfile;
using Turboapi.Auth.Domain.Interfaces;

namespace Turboapi.Auth.Presentation.Controllers
{
    [Route("api/auth/[controller]")]
    [Authorize(AuthenticationSchemes = $"{CookieAuthenticationDefaults.AuthenticationScheme},{JwtBearerDefaults.AuthenticationScheme}")]
    public class ProfileController : BaseApiController
    {
        private readonly ICommandHandler<UpdateProfileCommand, Result<ProfileResponse, UpdateProfileError>> _updateProfileHandler;
        private readonly IAccountRepository _accountRepository;

        public ProfileController(
            ICommandHandler<UpdateProfileCommand, Result<ProfileResponse, UpdateProfileError>> updateProfileHandler,
            IAccountRepository accountRepository)
        {
            _updateProfileHandler = updateProfileHandler;
            _accountRepository = accountRepository;
        }

        /// <summary>Returns the authenticated user's profile (email + display name).</summary>
        [HttpGet]
        [ProducesResponseType(typeof(ProfileResponse), StatusCodes.Status200OK)]
        [ProducesResponseType(StatusCodes.Status401Unauthorized)]
        public async Task<IActionResult> GetProfile()
        {
            if (!TryGetAccountId(out var accountId))
            {
                return Unauthorized("Invalid token format: 'sub' claim is not a valid GUID.");
            }

            var account = await _accountRepository.GetByIdAsync(accountId);
            if (account == null)
            {
                return Unauthorized();
            }

            return Ok(new ProfileResponse(account.Id, account.Email, account.DisplayName));
        }

        /// <summary>Updates the authenticated user's display name.</summary>
        [HttpPut]
        [ProducesResponseType(typeof(ProfileResponse), StatusCodes.Status200OK)]
        [ProducesResponseType(StatusCodes.Status400BadRequest)]
        [ProducesResponseType(StatusCodes.Status401Unauthorized)]
        public async Task<IActionResult> UpdateProfile([FromBody] UpdateProfileRequest request)
        {
            if (!TryGetAccountId(out var accountId))
            {
                return Unauthorized("Invalid token format: 'sub' claim is not a valid GUID.");
            }

            var command = new UpdateProfileCommand(accountId, request.DisplayName);
            var result = await _updateProfileHandler.Handle(command, HttpContext.RequestAborted);
            return HandleResult(result);
        }

        private bool TryGetAccountId(out Guid accountId)
        {
            var accountIdClaim = User.FindFirstValue(ClaimTypes.NameIdentifier)
                                 ?? User.FindFirstValue("sub");
            return Guid.TryParse(accountIdClaim, out accountId);
        }
    }
}
