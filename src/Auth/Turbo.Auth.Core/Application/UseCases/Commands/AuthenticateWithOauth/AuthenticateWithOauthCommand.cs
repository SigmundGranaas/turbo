namespace Turboapi.Auth.Application.UseCases.Commands.AuthenticateWithOAuth
{
    public record AuthenticateWithOAuthCommand(
        string ProviderName,
        string AuthorizationCode,
        string? RedirectUri, // Optional, for providers that require it to match the initial request
        string? State // Optional, for state validation if used
    );
}