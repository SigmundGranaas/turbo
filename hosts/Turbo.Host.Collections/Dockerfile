FROM mcr.microsoft.com/dotnet/aspnet:10.0 AS base
USER $APP_UID
WORKDIR /app
EXPOSE 8080

FROM mcr.microsoft.com/dotnet/sdk:10.0 AS build
ARG BUILD_CONFIGURATION=Release
WORKDIR /src

COPY ["Directory.Packages.props", "./"]
COPY ["hosts/Turbo.Host.Collections/Turbo.Host.Collections.csproj", "hosts/Turbo.Host.Collections/"]
COPY ["src/Collections/Turbo.Collections.Api/Turbo.Collections.Api.csproj", "src/Collections/Turbo.Collections.Api/"]
COPY ["src/Collections/Turbo.Collections.Core/Turbo.Collections.Core.csproj", "src/Collections/Turbo.Collections.Core/"]
COPY ["src/Collections/Turbo.Collections.Contracts/Turbo.Collections.Contracts.csproj", "src/Collections/Turbo.Collections.Contracts/"]
COPY ["src/Collections/Turbo.Collections.Infrastructure/Turbo.Collections.Infrastructure.csproj", "src/Collections/Turbo.Collections.Infrastructure/"]
COPY ["TurboAuthentication/TurboAuthentication.csproj", "TurboAuthentication/"]
COPY ["src/Shared/Turbo.Messaging.Abstractions/Turbo.Messaging.Abstractions.csproj", "src/Shared/Turbo.Messaging.Abstractions/"]
COPY ["src/Shared/Turbo.Messaging.Nats/Turbo.Messaging.Nats.csproj", "src/Shared/Turbo.Messaging.Nats/"]
COPY ["src/Shared/Turbo.Outbox.Abstractions/Turbo.Outbox.Abstractions.csproj", "src/Shared/Turbo.Outbox.Abstractions/"]
COPY ["src/Shared/Turbo.Outbox.Postgres/Turbo.Outbox.Postgres.csproj", "src/Shared/Turbo.Outbox.Postgres/"]
COPY ["src/Shared/Turbo.Hosting.Postgres/Turbo.Hosting.Postgres.csproj", "src/Shared/Turbo.Hosting.Postgres/"]
RUN dotnet restore "hosts/Turbo.Host.Collections/Turbo.Host.Collections.csproj"

COPY . .
WORKDIR /src/hosts/Turbo.Host.Collections
RUN dotnet publish "Turbo.Host.Collections.csproj" -c $BUILD_CONFIGURATION -o /app/publish /p:UseAppHost=false

FROM base AS final
WORKDIR /app
COPY --from=build /app/publish .
ENTRYPOINT ["dotnet", "Turbo.Host.Collections.dll"]
