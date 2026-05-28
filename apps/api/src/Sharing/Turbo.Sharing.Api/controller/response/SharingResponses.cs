using Turboapi.Sharing.domain.service;

namespace Turboapi.Sharing.controller.response;

public sealed record ErrorResponse(string Title, string Detail);
