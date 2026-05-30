using System.Security.Claims;
using Microsoft.AspNetCore.Authentication.JwtBearer;
using Microsoft.AspNetCore.Authorization;
using Microsoft.AspNetCore.Mvc;
using Turboapi.Auth.Application.Contracts.V1.Notifications;
using Turboapi.Auth.Domain.Interfaces;

namespace Turboapi.Auth.Presentation.Controllers
{
    /// <summary>
    /// Registration of push-notification device tokens for the authenticated
    /// user. The client registers its FCM/APNs token after sign-in and on
    /// token refresh, and unregisters on logout.
    /// </summary>
    [ApiController]
    [Route("api/auth/[controller]")]
    [Authorize(AuthenticationSchemes = $"{Microsoft.AspNetCore.Authentication.Cookies.CookieAuthenticationDefaults.AuthenticationScheme},{JwtBearerDefaults.AuthenticationScheme}")]
    public class DevicesController : ControllerBase
    {
        private readonly IDeviceTokenRepository _deviceTokens;

        public DevicesController(IDeviceTokenRepository deviceTokens)
        {
            _deviceTokens = deviceTokens;
        }

        [HttpPost]
        [ProducesResponseType(StatusCodes.Status204NoContent)]
        [ProducesResponseType(StatusCodes.Status400BadRequest)]
        [ProducesResponseType(StatusCodes.Status401Unauthorized)]
        public async Task<IActionResult> Register([FromBody] RegisterDeviceRequest request)
        {
            if (string.IsNullOrWhiteSpace(request.Token))
            {
                return BadRequest("Token is required.");
            }
            if (!TryGetAccountId(out var accountId))
            {
                return Unauthorized();
            }

            await _deviceTokens.RegisterAsync(accountId, request.Token, request.Platform, HttpContext.RequestAborted);
            return NoContent();
        }

        [HttpPost("unregister")]
        [ProducesResponseType(StatusCodes.Status204NoContent)]
        [ProducesResponseType(StatusCodes.Status401Unauthorized)]
        public async Task<IActionResult> Unregister([FromBody] UnregisterDeviceRequest request)
        {
            if (!TryGetAccountId(out _))
            {
                return Unauthorized();
            }
            if (!string.IsNullOrWhiteSpace(request.Token))
            {
                await _deviceTokens.RemoveAsync(request.Token, HttpContext.RequestAborted);
            }
            return NoContent();
        }

        private bool TryGetAccountId(out Guid accountId)
        {
            var accountIdClaim = User.FindFirstValue(ClaimTypes.NameIdentifier)
                                 ?? User.FindFirstValue("sub");
            return Guid.TryParse(accountIdClaim, out accountId);
        }
    }
}
