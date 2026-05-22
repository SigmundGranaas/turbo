namespace Turboapi.Auth.Application.Contracts.V1.Tokens
{
    public record TokenResult(
        string AccessToken,
        string RefreshToken,
        Guid AccountId
    );

    public record RefreshTokenRequest(string? RefreshToken);
}