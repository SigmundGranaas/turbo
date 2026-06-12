FROM mcr.microsoft.com/dotnet/aspnet:10.0 AS base
USER $APP_UID
WORKDIR /app
EXPOSE 8080

FROM mcr.microsoft.com/dotnet/sdk:10.0 AS build
ARG BUILD_CONFIGURATION=Release
WORKDIR /src

COPY ["Directory.Packages.props", "./"]
COPY ["hosts/Turbo.Host.Places/Turbo.Host.Places.csproj", "hosts/Turbo.Host.Places/"]
COPY ["src/Places/Turbo.Places.Api/Turbo.Places.Api.csproj", "src/Places/Turbo.Places.Api/"]
COPY ["src/Places/Turbo.Places.Core/Turbo.Places.Core.csproj", "src/Places/Turbo.Places.Core/"]
COPY ["src/Places/Turbo.Places.Contracts/Turbo.Places.Contracts.csproj", "src/Places/Turbo.Places.Contracts/"]
COPY ["src/Places/Turbo.Places.Infrastructure/Turbo.Places.Infrastructure.csproj", "src/Places/Turbo.Places.Infrastructure/"]
RUN dotnet restore "hosts/Turbo.Host.Places/Turbo.Host.Places.csproj"

COPY . .
WORKDIR /src/hosts/Turbo.Host.Places
RUN dotnet publish "Turbo.Host.Places.csproj" -c $BUILD_CONFIGURATION -o /app/publish /p:UseAppHost=false

FROM base AS final
WORKDIR /app
COPY --from=build /app/publish .
# place-core cdylib (staged into the context by api_image_build.yaml).
COPY native/ /app/native/
ENV PLACE_CORE_LIB=/app/native
ENTRYPOINT ["dotnet", "Turbo.Host.Places.dll"]
