# Turboapi Architecture Guide

## System Architecture

Turboapi is a .NET 10 system that can be deployed either as a modulith
(`Turbo.Host.Modulith`, one process binding all three modules) or as
three independent microservices behind a YARP gateway. The same module
code runs under both topologies.

Each module ships as four assemblies (`Core` / `Contracts` /
`Infrastructure` / `Api`):

- **`src/Auth/`** — authentication (registration, login, tokens, OAuth).
- **`src/Geo/`** — location service with PostGIS geospatial capabilities.
- **`src/Activity/`** — activity management.

Cross-cutting libraries:

- **`src/Shared/Turbo.Messaging.*`** — `IDomainEvent` + transport
  abstractions; NATS JetStream and in-process implementations.
- **`src/Shared/Turbo.Outbox.*`** — transactional outbox abstractions
  plus the Postgres dispatcher.
- **`src/Gateway/`** — YARP reverse proxy that swaps between
  microservice and modulith routing via a `Topology` env var.
- **`src/Shared/Turbo.Hosting.Postgres/`** — host-startup helper that
  creates each module's database if missing and runs EF Core migrations
  via `MigrateModuleDatabaseAsync<TContext>`.
- **`TurboAuthentication/`** — shared JWT/cookie scheme used by Geo and
  Activity to validate Auth-issued tokens.

### Key Architecture Patterns

- **Hexagonal / Ports-and-Adapters**: Core depends only on
  Messaging.Abstractions and Outbox.Abstractions; EF Core, Npgsql,
  NATS, ASP.NET live in Infrastructure / Api. `tests/Turbo.Architecture/`
  enforces this with NetArchTest.
- **Domain-Driven Design**: each module is a bounded context.
- **CQRS**: command handlers (write side) drain aggregate domain events
  into the transactional outbox; query handlers read from a read-model
  projection populated by subscriber handlers.
- **Transactional outbox + at-least-once delivery**: events flow through
  the outbox in the same DB transaction as the aggregate write; an
  `OutboxDispatcherHostedService` publishes to the active transport
  (NATS or in-process).
- **Symmetric deploy topology**: dedicated DBs (default) or shared
  Postgres, dedicated microservices or modulith — chosen at deploy time
  via compose / k8s overlays, never via app code.

### Infrastructure

- **Docker**: Containerized services
- **NATS JetStream**: Event streaming backbone (with transactional outbox)
- **PostgreSQL**: Database with extensions (PostGIS)
- **EF Core Migrations**: per-module, applied in-process at host startup
- **OpenTelemetry/Prometheus/Grafana**: Monitoring stack

## Domain Modeling

- **Aggregates**: Core domain entities (Location, Activity, User)
- **Value Objects**: Immutable objects without identity (LatLng)
- **Domain Events**: Rich event model for cross-service consistency
- **Commands**: Represent user intent to change system state
- **Queries**: Read-only operations against materialized views

## Testing Approaches

1. **Unit Tests**
   - Domain model and business logic testing
   - Command/query handler tests for interfacing with the domain
   - No mocking. Object graphs are alwways reconstructed, using only mocks for out of process dependencies

2. **Integration Tests**
   - Database integration with Testcontainers
   - NATS JetStream integration for event handling (via outbox)
   - API endpoint testing

3. **Performance Tests (k6)**
   - Load testing scripts in `/performance/k6`
   - Stress testing for limits

## Coding Practices

1. **Clean Architecture**
   - Domain core isolated from infrastructure
   - Dependency inversion with interfaces

2. **SOLID Principles**
   - Single Responsibility: Focused classes
   - Interface Segregation: Focused interfaces
   - Dependency Inversion: Dependency injection

3. **Error Handling**
   - Domain-specific exceptions
   - Global exception middleware

4. **Observability**
   - Structured logging
   - Distributed tracing
   - Metrics collection

5. **Security**
   - JWT authentication. (User id is embedded into the key)
   - Authorization middleware