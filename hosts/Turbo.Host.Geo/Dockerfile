FROM mcr.microsoft.com/dotnet/aspnet:10.0 AS base
USER $APP_UID
WORKDIR /app
EXPOSE 8080

FROM mcr.microsoft.com/dotnet/sdk:10.0 AS build
ARG BUILD_CONFIGURATION=Release
WORKDIR /src

COPY ["Directory.Packages.props", "./"]
COPY ["hosts/Turbo.Host.Geo/Turbo.Host.Geo.csproj", "hosts/Turbo.Host.Geo/"]
COPY ["src/Geo/Turbo.Geo.Api/Turbo.Geo.Api.csproj", "src/Geo/Turbo.Geo.Api/"]
COPY ["src/Geo/Turbo.Geo.Core/Turbo.Geo.Core.csproj", "src/Geo/Turbo.Geo.Core/"]
COPY ["src/Geo/Turbo.Geo.Contracts/Turbo.Geo.Contracts.csproj", "src/Geo/Turbo.Geo.Contracts/"]
COPY ["src/Geo/Turbo.Geo.Infrastructure/Turbo.Geo.Infrastructure.csproj", "src/Geo/Turbo.Geo.Infrastructure/"]
COPY ["TurboAuthentication/TurboAuthentication.csproj", "TurboAuthentication/"]
COPY ["Turbo-pg-data/Turbo-pg-data.csproj", "Turbo-pg-data/"]
COPY ["src/Shared/Turbo.Messaging.Abstractions/Turbo.Messaging.Abstractions.csproj", "src/Shared/Turbo.Messaging.Abstractions/"]
COPY ["src/Shared/Turbo.Messaging.Nats/Turbo.Messaging.Nats.csproj", "src/Shared/Turbo.Messaging.Nats/"]
COPY ["src/Shared/Turbo.Outbox.Abstractions/Turbo.Outbox.Abstractions.csproj", "src/Shared/Turbo.Outbox.Abstractions/"]
COPY ["src/Shared/Turbo.Outbox.Postgres/Turbo.Outbox.Postgres.csproj", "src/Shared/Turbo.Outbox.Postgres/"]
RUN dotnet restore "hosts/Turbo.Host.Geo/Turbo.Host.Geo.csproj"

COPY . .
WORKDIR /src/hosts/Turbo.Host.Geo
RUN dotnet publish "Turbo.Host.Geo.csproj" -c $BUILD_CONFIGURATION -o /app/publish /p:UseAppHost=false

FROM base AS final
WORKDIR /app
COPY --from=build /app/publish .
ENTRYPOINT ["dotnet", "Turbo.Host.Geo.dll"]
