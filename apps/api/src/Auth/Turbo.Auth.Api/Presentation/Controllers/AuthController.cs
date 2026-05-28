using System.Security.Claims;
using Microsoft.AspNetCore.Authentication.JwtBearer;
using Microsoft.AspNetCore.Authorization;
using Microsoft.AspNetCore.Mvc;
using Microsoft.Extensions.Options;
using Turboapi.Auth.Application.Contracts.V1.Auth;
using Turboapi.Auth.Application.Interfaces;
using Turboapi.Auth.Application.Results;
using Turboapi.Auth.Application.Results.Errors;
using Turboapi.Auth.Application.UseCases.Commands.ChangePassword;
using Turboapi.Auth.Application.UseCases.Commands.LoginUserWithPassword;
using Turboapi.Auth.Application.UseCases.Commands.RegisterUserWithPassword;
using Turboapi.Auth.Infrastructure.Auth;
using Turboapi.Auth.Presentation.Cookies;

namespace Turboapi.Auth.Presentation.Controllers
{
    [Route("api/auth/[controller]")] 
    public class AuthController : BaseApiController
    {
        private readonly ICommandHandler<RegisterUserWithPasswordCommand, Result<AuthTokenResponse, RegistrationError>> _registerHandler;
        private readonly ICommandHandler<LoginUserWithPasswordCommand, Result<AuthTokenResponse, LoginError>> _loginHandler;
        private readonly ICommandHandler<ChangePasswordCommand, Result<ChangePasswordError>> _changePasswordHandler;
        private readonly ICookieManager _cookieManager;
        private readonly JwtConfig _jwtConfig;

        public AuthController(
            ICommandHandler<RegisterUserWithPasswordCommand, Result<AuthTokenResponse, RegistrationError>> registerHandler,
            ICommandHandler<LoginUserWithPasswordCommand, Result<AuthTokenResponse, LoginError>> loginHandler,
            ICommandHandler<ChangePasswordCommand, Result<ChangePasswordError>> changePasswordHandler,
            ICookieManager cookieManager,
            IOptions<JwtConfig> jwtConfig)
        {
            _registerHandler = registerHandler;
            _loginHandler = loginHandler;
            _changePasswordHandler = changePasswordHandler;
            _cookieManager = cookieManager;
            _jwtConfig = jwtConfig.Value;
        }

        [HttpPost("register")]
        public async Task<IActionResult> Register([FromBody] RegisterUserWithPasswordRequest request)
        {
            if (request.Password != request.ConfirmPassword)
            {
                // This is now handled by the error mapping in BaseApiController for consistency.
                return HandleResult<AuthTokenResponse, RegistrationError>(RegistrationError.InvalidInput);
            }
            var command = new RegisterUserWithPasswordCommand(request.Email, request.Password);
            var result = await _registerHandler.Handle(command, HttpContext.RequestAborted);
            
            result.Switch(
                success => _cookieManager.SetAuthCookies(success.AccessToken, success.RefreshToken, _jwtConfig.TokenExpirationMinutes),
                failure => {}
            );

            return HandleResult(result);
        }

        [HttpPost("login")]
        public async Task<IActionResult> Login([FromBody] LoginUserWithPasswordRequest request)
        {
            var command = new LoginUserWithPasswordCommand(request.Email, request.Password);
            var result = await _loginHandler.Handle(command, HttpContext.RequestAborted);
            
            result.Switch(
                success => _cookieManager.SetAuthCookies(success.AccessToken, success.RefreshToken, _jwtConfig.TokenExpirationMinutes),
                failure => {}
            );

            return HandleResult(result);
        }

        /// <summary>
        /// Changes the authenticated user's password. Requires the current
        /// password. Accounts without a password method (e.g. Google sign-in)
        /// are rejected with 403, since there is no password to change.
        /// </summary>
        [HttpPost("change-password")]
        [Authorize(AuthenticationSchemes = $"{Microsoft.AspNetCore.Authentication.Cookies.CookieAuthenticationDefaults.AuthenticationScheme},{JwtBearerDefaults.AuthenticationScheme}")]
        [ProducesResponseType(StatusCodes.Status204NoContent)]
        [ProducesResponseType(StatusCodes.Status400BadRequest)]
        [ProducesResponseType(StatusCodes.Status401Unauthorized)]
        [ProducesResponseType(StatusCodes.Status403Forbidden)]
        public async Task<IActionResult> ChangePassword([FromBody] ChangePasswordRequest request)
        {
            if (request.NewPassword != request.ConfirmNewPassword)
            {
                return HandleResult(Result.Failure(ChangePasswordError.InvalidInput));
            }

            var accountIdClaim = User.FindFirstValue(ClaimTypes.NameIdentifier)
                                 ?? User.FindFirstValue("sub");
            if (!Guid.TryParse(accountIdClaim, out var accountId))
            {
                return Unauthorized("Invalid token format: 'sub' claim is not a valid GUID.");
            }

            var command = new ChangePasswordCommand(accountId, request.CurrentPassword, request.NewPassword);
            var result = await _changePasswordHandler.Handle(command, HttpContext.RequestAborted);
            return HandleResult(result);
        }
    }
}