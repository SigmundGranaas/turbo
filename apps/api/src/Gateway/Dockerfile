FROM mcr.microsoft.com/dotnet/aspnet:10.0 AS base
USER $APP_UID
WORKDIR /app
EXPOSE 8080

FROM mcr.microsoft.com/dotnet/sdk:10.0 AS build
ARG BUILD_CONFIGURATION=Release
WORKDIR /src

COPY ["Directory.Packages.props", "./"]
COPY ["src/Gateway/Turbo.Gateway.csproj", "src/Gateway/"]
RUN dotnet restore "src/Gateway/Turbo.Gateway.csproj"

COPY . .
WORKDIR /src/src/Gateway
RUN dotnet publish "Turbo.Gateway.csproj" -c $BUILD_CONFIGURATION -o /app/publish /p:UseAppHost=false

FROM base AS final
WORKDIR /app
COPY --from=build /app/publish .
ENTRYPOINT ["dotnet", "Turbo.Gateway.dll"]
