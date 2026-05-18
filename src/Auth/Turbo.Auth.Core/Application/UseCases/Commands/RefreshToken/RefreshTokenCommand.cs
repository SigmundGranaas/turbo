namespace Turboapi.Auth.Application.UseCases.Commands.RefreshToken
{
    public record RefreshTokenCommand(
        string RefreshTokenString
    );
}