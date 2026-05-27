FROM mcr.microsoft.com/dotnet/aspnet:10.0 AS base
USER $APP_UID
WORKDIR /app
EXPOSE 8080

FROM mcr.microsoft.com/dotnet/sdk:10.0 AS build
ARG BUILD_CONFIGURATION=Release
WORKDIR /src

COPY ["Directory.Packages.props", "./"]
COPY ["hosts/Turbo.Host.Sharing/Turbo.Host.Sharing.csproj", "hosts/Turbo.Host.Sharing/"]
COPY ["src/Sharing/Turbo.Sharing.Api/Turbo.Sharing.Api.csproj", "src/Sharing/Turbo.Sharing.Api/"]
COPY ["src/Sharing/Turbo.Sharing.Core/Turbo.Sharing.Core.csproj", "src/Sharing/Turbo.Sharing.Core/"]
COPY ["src/Sharing/Turbo.Sharing.Contracts/Turbo.Sharing.Contracts.csproj", "src/Sharing/Turbo.Sharing.Contracts/"]
COPY ["src/Sharing/Turbo.Sharing.Infrastructure/Turbo.Sharing.Infrastructure.csproj", "src/Sharing/Turbo.Sharing.Infrastructure/"]
COPY ["src/Collections/Turbo.Collections.Contracts/Turbo.Collections.Contracts.csproj", "src/Collections/Turbo.Collections.Contracts/"]
COPY ["src/Geo/Turbo.Geo.Contracts/Turbo.Geo.Contracts.csproj", "src/Geo/Turbo.Geo.Contracts/"]
COPY ["src/Tracks/Turbo.Tracks.Contracts/Turbo.Tracks.Contracts.csproj", "src/Tracks/Turbo.Tracks.Contracts/"]
COPY ["TurboAuthentication/TurboAuthentication.csproj", "TurboAuthentication/"]
COPY ["src/Shared/Turbo.Messaging.Abstractions/Turbo.Messaging.Abstractions.csproj", "src/Shared/Turbo.Messaging.Abstractions/"]
COPY ["src/Shared/Turbo.Messaging.Nats/Turbo.Messaging.Nats.csproj", "src/Shared/Turbo.Messaging.Nats/"]
COPY ["src/Shared/Turbo.Outbox.Abstractions/Turbo.Outbox.Abstractions.csproj", "src/Shared/Turbo.Outbox.Abstractions/"]
COPY ["src/Shared/Turbo.Outbox.Postgres/Turbo.Outbox.Postgres.csproj", "src/Shared/Turbo.Outbox.Postgres/"]
COPY ["src/Shared/Turbo.Hosting.Postgres/Turbo.Hosting.Postgres.csproj", "src/Shared/Turbo.Hosting.Postgres/"]
RUN dotnet restore "hosts/Turbo.Host.Sharing/Turbo.Host.Sharing.csproj"

COPY . .
WORKDIR /src/hosts/Turbo.Host.Sharing
RUN dotnet publish "Turbo.Host.Sharing.csproj" -c $BUILD_CONFIGURATION -o /app/publish /p:UseAppHost=false

FROM base AS final
WORKDIR /app
COPY --from=build /app/publish .
ENTRYPOINT ["dotnet", "Turbo.Host.Sharing.dll"]
