namespace Turboapi.Auth.Application.Contracts.V1.Notifications
{
    public record RegisterDeviceRequest(
        string Token,
        string Platform);

    public record UnregisterDeviceRequest(
        string Token);
}
