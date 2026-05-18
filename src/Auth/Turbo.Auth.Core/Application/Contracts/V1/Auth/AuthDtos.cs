namespace Turboapi.Auth.Application.Contracts.V1.Auth
{
    public record RegisterUserWithPasswordRequest(
        string Email,
        string Password,
        string ConfirmPassword 
    );

    public record LoginUserWithPasswordRequest(
        string Email,
        string Password
    );

    public record AuthTokenResponse(
        string AccessToken,
        string RefreshToken,
        Guid AccountId,
        string Email
    );
}