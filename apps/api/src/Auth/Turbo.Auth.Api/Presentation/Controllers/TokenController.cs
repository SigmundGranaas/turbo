using Microsoft.AspNetCore.Mvc;
using Microsoft.Extensions.Options;
using Turboapi.Auth.Application.Contracts.V1.Auth;
using Turboapi.Auth.Application.Contracts.V1.Common;
using Turboapi.Auth.Application.Contracts.V1.Tokens;
using Turboapi.Auth.Application.Interfaces;
using Turboapi.Auth.Application.Results;
using Turboapi.Auth.Application.Results.Errors;
using Turboapi.Auth.Application.UseCases.Commands.RefreshToken;
using Turboapi.Auth.Application.UseCases.Commands.RevokeRefreshToken;
using Turboapi.Auth.Infrastructure.Auth;
using Turboapi.Auth.Presentation.Cookies;

namespace Turboapi.Auth.Presentation.Controllers
{
    [Route("api/auth/[controller]")]
    public class TokenController : BaseApiController
    {
        private readonly ICommandHandler<RefreshTokenCommand, Result<AuthTokenResponse, RefreshTokenError>> _refreshHandler;
        private readonly ICommandHandler<RevokeRefreshTokenCommand, Result<RefreshTokenError>> _revokeHandler;
        private readonly ICookieManager _cookieManager;
        private readonly JwtConfig _jwtConfig;

        public TokenController(
            ICommandHandler<RefreshTokenCommand, Result<AuthTokenResponse, RefreshTokenError>> refreshHandler,
            ICommandHandler<RevokeRefreshTokenCommand, Result<RefreshTokenError>> revokeHandler,
            ICookieManager cookieManager,
            IOptions<JwtConfig> jwtConfig)
        {
            _refreshHandler = refreshHandler;
            _revokeHandler = revokeHandler;
            _cookieManager = cookieManager;
            _jwtConfig = jwtConfig.Value;
        }

        [HttpPost("refresh")]
        public async Task<IActionResult> Refresh([FromBody(EmptyBodyBehavior = Microsoft.AspNetCore.Mvc.ModelBinding.EmptyBodyBehavior.Allow)] RefreshTokenRequest? request)
        {
            var tokenToRefresh = request?.RefreshToken ?? _cookieManager.GetRefreshToken();

            if (string.IsNullOrWhiteSpace(tokenToRefresh))
            {
                return Unauthorized(new ErrorResponse("RefreshTokenError", "MissingRefreshToken"));
            }

            var command = new RefreshTokenCommand(tokenToRefresh);
            var result = await _refreshHandler.Handle(command, HttpContext.RequestAborted);
            
            result.Switch(
                success => _cookieManager.SetAuthCookies(success.AccessToken, success.RefreshToken, _jwtConfig.TokenExpirationMinutes),
                failure => { if (failure is RefreshTokenError.Revoked or RefreshTokenError.InvalidToken) _cookieManager.ClearAuthCookies(); }
            );

            return HandleResult(result);
        }

        [HttpPost("revoke")]
        public async Task<IActionResult> Revoke([FromBody] RevokeTokenRequest? request)
        {
            var tokenToRevoke = request?.RefreshToken ?? _cookieManager.GetRefreshToken();
            
            _cookieManager.ClearAuthCookies();

            if (string.IsNullOrWhiteSpace(tokenToRevoke))
            {
                return NoContent();
            }

            var command = new RevokeRefreshTokenCommand(tokenToRevoke);
            var result = await _revokeHandler.Handle(command, HttpContext.RequestAborted);
            
            return HandleResult(result);
        }
    }
}