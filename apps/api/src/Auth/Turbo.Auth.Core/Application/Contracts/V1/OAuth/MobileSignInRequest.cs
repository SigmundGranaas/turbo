namespace Turboapi.Auth.Application.Contracts.V1.OAuth;

/// <summary>
/// DTO for the mobile-specific sign-in endpoint.
/// </summary>
public record MobileSignInRequest(
    string Provider,
    string Code,
    string? State
);