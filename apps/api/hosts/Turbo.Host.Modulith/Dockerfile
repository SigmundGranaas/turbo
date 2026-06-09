FROM mcr.microsoft.com/dotnet/aspnet:10.0 AS base
USER $APP_UID
WORKDIR /app
EXPOSE 8080

FROM mcr.microsoft.com/dotnet/sdk:10.0 AS build
ARG BUILD_CONFIGURATION=Release
WORKDIR /src

COPY ["Directory.Packages.props", "./"]
COPY ["hosts/Turbo.Host.Modulith/Turbo.Host.Modulith.csproj", "hosts/Turbo.Host.Modulith/"]
COPY ["src/Auth/Turbo.Auth.Api/Turbo.Auth.Api.csproj", "src/Auth/Turbo.Auth.Api/"]
COPY ["src/Auth/Turbo.Auth.Core/Turbo.Auth.Core.csproj", "src/Auth/Turbo.Auth.Core/"]
COPY ["src/Auth/Turbo.Auth.Contracts/Turbo.Auth.Contracts.csproj", "src/Auth/Turbo.Auth.Contracts/"]
COPY ["src/Auth/Turbo.Auth.Infrastructure/Turbo.Auth.Infrastructure.csproj", "src/Auth/Turbo.Auth.Infrastructure/"]
COPY ["src/Tracks/Turbo.Tracks.Api/Turbo.Tracks.Api.csproj", "src/Tracks/Turbo.Tracks.Api/"]
COPY ["src/Tracks/Turbo.Tracks.Core/Turbo.Tracks.Core.csproj", "src/Tracks/Turbo.Tracks.Core/"]
COPY ["src/Tracks/Turbo.Tracks.Contracts/Turbo.Tracks.Contracts.csproj", "src/Tracks/Turbo.Tracks.Contracts/"]
COPY ["src/Tracks/Turbo.Tracks.Infrastructure/Turbo.Tracks.Infrastructure.csproj", "src/Tracks/Turbo.Tracks.Infrastructure/"]
COPY ["src/Geo/Turbo.Geo.Api/Turbo.Geo.Api.csproj", "src/Geo/Turbo.Geo.Api/"]
COPY ["src/Geo/Turbo.Geo.Core/Turbo.Geo.Core.csproj", "src/Geo/Turbo.Geo.Core/"]
COPY ["src/Geo/Turbo.Geo.Contracts/Turbo.Geo.Contracts.csproj", "src/Geo/Turbo.Geo.Contracts/"]
COPY ["src/Geo/Turbo.Geo.Infrastructure/Turbo.Geo.Infrastructure.csproj", "src/Geo/Turbo.Geo.Infrastructure/"]
COPY ["src/Places/Turbo.Places.Api/Turbo.Places.Api.csproj", "src/Places/Turbo.Places.Api/"]
COPY ["src/Places/Turbo.Places.Core/Turbo.Places.Core.csproj", "src/Places/Turbo.Places.Core/"]
COPY ["src/Places/Turbo.Places.Contracts/Turbo.Places.Contracts.csproj", "src/Places/Turbo.Places.Contracts/"]
COPY ["src/Places/Turbo.Places.Infrastructure/Turbo.Places.Infrastructure.csproj", "src/Places/Turbo.Places.Infrastructure/"]
COPY ["TurboAuthentication/TurboAuthentication.csproj", "TurboAuthentication/"]
COPY ["src/Shared/Turbo.Messaging.Abstractions/Turbo.Messaging.Abstractions.csproj", "src/Shared/Turbo.Messaging.Abstractions/"]
COPY ["src/Shared/Turbo.Messaging.InProcess/Turbo.Messaging.InProcess.csproj", "src/Shared/Turbo.Messaging.InProcess/"]
COPY ["src/Shared/Turbo.Messaging.Nats/Turbo.Messaging.Nats.csproj", "src/Shared/Turbo.Messaging.Nats/"]
COPY ["src/Shared/Turbo.Outbox.Abstractions/Turbo.Outbox.Abstractions.csproj", "src/Shared/Turbo.Outbox.Abstractions/"]
COPY ["src/Shared/Turbo.Outbox.Postgres/Turbo.Outbox.Postgres.csproj", "src/Shared/Turbo.Outbox.Postgres/"]
COPY ["src/Shared/Turbo.Hosting.Postgres/Turbo.Hosting.Postgres.csproj", "src/Shared/Turbo.Hosting.Postgres/"]
RUN dotnet restore "hosts/Turbo.Host.Modulith/Turbo.Host.Modulith.csproj"

COPY . .
WORKDIR /src/hosts/Turbo.Host.Modulith
RUN dotnet publish "Turbo.Host.Modulith.csproj" -c $BUILD_CONFIGURATION -o /app/publish /p:UseAppHost=false

FROM base AS final
WORKDIR /app
COPY --from=build /app/publish .
# place-core cdylib for the Places module (staged into the context by
# api_image_build.yaml; native/.gitkeep keeps the COPY valid locally).
COPY native/ /app/native/
ENV PLACE_CORE_LIB=/app/native
ENTRYPOINT ["dotnet", "Turbo.Host.Modulith.dll"]
