using System.Security.Claims;
using Microsoft.AspNetCore.Authentication.Cookies;
using Microsoft.AspNetCore.Authentication.JwtBearer;
using Microsoft.AspNetCore.Authorization;
using Microsoft.AspNetCore.Mvc;
using Turboapi.Auth.Application.UseCases.Queries.ValidateSession;
using Turboapi.Auth.Domain.Interfaces;

namespace Turboapi.Auth.Presentation.Controllers
{
    [Route("api/auth/[controller]")]
    [Authorize(AuthenticationSchemes = $"{CookieAuthenticationDefaults.AuthenticationScheme},{JwtBearerDefaults.AuthenticationScheme}")]
    public class SessionController : BaseApiController
    {
        private readonly IAccountRepository _accountRepository;

        public SessionController(IAccountRepository accountRepository)
        {
            _accountRepository = accountRepository;
        }

        [HttpGet("me")]
        [ProducesResponseType(typeof(ValidateSessionResponse), StatusCodes.Status200OK)]
        [ProducesResponseType(StatusCodes.Status401Unauthorized)]
        [ProducesResponseType(StatusCodes.Status403Forbidden)]
        public async Task<IActionResult> GetCurrentUser()
        {
            var accountIdClaim = User.FindFirstValue(ClaimTypes.NameIdentifier) 
                                 ?? User.FindFirstValue("sub");

            if (!Guid.TryParse(accountIdClaim, out var accountId))
            {
                return Unauthorized("Invalid token format: 'sub' claim is not a valid GUID.");
            }

            var account = await _accountRepository.GetByIdAsync(accountId);
            if (account == null)
            {
                return Unauthorized();
            }

            if (!account.IsActive)
            {
                return Forbid();
            }

            var response = new ValidateSessionResponse(
                accountId,
                User.FindFirstValue(ClaimTypes.Email) ?? string.Empty,
                User.FindAll(ClaimTypes.Role).Select(c => c.Value).ToList(),
                account.IsActive,
                account.DisplayName
            );

            return Ok(response);
        }
    }
}