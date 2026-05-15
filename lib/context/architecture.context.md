# **Turbo Map App: Architectural Guidelines**

## 1. Vision & Purpose

This document outlines the architectural principles for the Turbo map application. The primary goal is to create a **scalable, maintainable, and highly testable** codebase by adopting a **Feature-Oriented Architecture**.

As the application grows, this structure will help us:
*   **Isolate Complexity:** Group all code related to a single piece of functionality (e.g., Authentication, Search) into a self-contained module.
*   **Enforce Clear Boundaries:** Prevent features from becoming tightly coupled, making them easier to modify or replace without causing cascading failures.
*   **Improve Developer Velocity:** Enable developers to work on different features in parallel with minimal merge conflicts.
*   **Enhance Testability:** Make it simple to test a feature's logic and UI in complete isolation.

## 2. Core Principles

1.  **Feature Encapsulation:** The application is a collection of independent "features." Each feature is responsible for one domain of the app.
2.  **Explicit API Boundaries:** Each feature exposes a public contract through a single `api.dart` file. **This is the only file that code outside the feature is allowed to import.** This is the cornerstone of our encapsulation strategy.
3.  **Single Responsibility:** Code is organized by its role. Core services are separated from application setup, which is separated from feature-specific logic.

## 3. The Directory Structure

The `lib` directory is organized into three main areas: `app`, `core`, and `features`.

```
/lib
├── app/          # Application shell: main entry, routing, theming
├── core/         # Shared code: API clients, generic widgets, persistence
└── features/     # Self-contained application features
```

### `app/` - The Application Shell

This directory contains the code that "hosts" the features.
*   `app.dart`: The main `MaterialApp` widget, responsible for theme, localization, and routing.
*   `app_theme.dart`: The application's `ThemeData`.
*   `l10n/`: All localization and internationalization files.
*   `main.dart`: The application's entry point. Its only job is to initialize bindings and run the app.

### `core/` - Shared, Cross-Cutting Concerns

This directory holds code that is truly generic and can be used by any feature, but is not tied to a specific feature's business logic.
*   `api/`: Contains the base `ApiClient` (Dio setup, interceptors).
*   `config/`: Environment configuration (`env_config.dart`).
*   `persistence/`: Abstract data storage interfaces (`MarkerDataStore`) and their concrete implementations (`SQLiteMarkerDataStore`, `IndexedDBMarkerDataStore`).
*   `providers/`: App-wide providers that are not specific to one feature, like the `markerDataStoreProvider`.
*   `widgets/`: Truly generic widgets used across multiple features (e.g., `AppDrawer`, `DevModeBanner`).

### `features/` - The Heart of the Application

Each subdirectory within `features/` is a self-contained module.

## 4. The Anatomy of a Feature

Every feature follows a consistent internal structure. Let's use the **`auth`** feature as an example.

```
/features
└── auth/
    ├── api.dart              <-- The ONLY public entry point to this feature.
    ├── data/                 <-- Business logic: notifiers, services.
    │   ├── auth_service.dart
    │   └── auth_state_notifier.dart
    ├── models/               <-- Plain data classes for this feature.
    │   └── auth_state.dart
    └── widgets/              <-- All UI components for this feature.
        ├── login_screen.dart
        └── ...
```

### The Public Contract: `api.dart`

This is the most important file in any feature. It defines the feature's public API.

`api.dart` is a thin re-export façade. It must contain only `export` directives (and optionally a `library;` declaration). Provider globals, notifier classes, and helper functions all live in `data/`; `api.dart` simply re-exports the names that outside code is allowed to use.

**What `api.dart` exports:**
1.  **Public state providers** — declared in the same `data/` file as their notifier, re-exported via `export ... show xxxProvider, XxxNotifier;`.
2.  **Public models** — any data classes that other features or the UI need to interact with.
3.  **Public UI entry points** — the main screen or widget for the feature (e.g., `LoginScreen`).

**The notifier is the public API.** Consumers call methods via `ref.read(xxxProvider.notifier).method(...)`. Do not introduce an `Api`/wrapper class around the notifier — it adds an indirection layer without buying testability, since notifiers are already mockable via `ProviderContainer.overrides`.

**Example: `features/auth/api.dart`**
```dart
/// The public API for the Auth feature.
library;

export 'data/auth_state_notifier.dart' show authStateProvider, AuthStateNotifier;
export 'models/auth_state.dart' show AuthState, AuthStatus;
export 'widgets/login_screen.dart' show LoginScreen;
```

### Provider type matrix

| Use case | Provider | `build()` returns |
|---|---|---|
| Synchronous state | `NotifierProvider<X, T>` | `T` |
| Naturally async state (loading flicker acceptable) | `AsyncNotifierProvider<X, T>` | `Future<T>` |
| Async with immediate seed + background fill | `NotifierProvider<X, AsyncValue<T>>` returning `AsyncValue.data(seed)` and triggering `_loadData()` from `build()` | `AsyncValue<T>` |
| Stateless DI / service singleton | `Provider<T>` | `T` |
| Stream source | `StreamProvider<T>` | `Stream<T>` |
| One-shot future, no methods | `FutureProvider<T>` | `Future<T>` |

The seeded-`AsyncValue` pattern (third row) is used **deliberately** by `LocationRepository`, `SavedPathRepository`, `ViewportMarkerNotifier`, and `ViewportSavedPathNotifier` to avoid a startup loading flicker. Do not "normalize" them to `AsyncNotifierProvider` — it would change observable behavior.

### No code generation

This codebase does not use `@riverpod`, `freezed`, or `build_runner`. State classes are immutable with hand-written `const` constructors and `copyWith`. Provider types are written out explicitly:

```dart
final myProvider = NotifierProvider<MyNotifier, MyState>(MyNotifier.new);
```

Do **not** re-introduce `riverpod_annotation` or `build_runner` in `pubspec.yaml`. The tradeoff is intentional: a few extra characters per provider, no generated artifacts to keep in sync.

### File naming (soft convention)

Existing files keep their current names. For new features, prefer `<feature>_notifier.dart` for the file that holds the main notifier and its provider global.

### The Private Implementation

*   `data/`: Contains the "brains" of the feature. This includes `AuthStateNotifier`, which orchestrates the logic, and `AuthService`, which handles direct communication with the backend endpoints. These are implementation details.
*   `models/`: Contains the plain data objects for the feature, like `AuthState` and `AuthStatus`.
*   `widgets/`: Contains all widgets and screens related to authentication, such as `LoginScreen`, `RegisterScreen`, `AuthTextField`, etc. These widgets should use the providers and models exposed in `api.dart`.

## 5. Practical Application: How-To Guides

### How to Create a New Feature (e.g., "Favorites")

1.  **Create the Directory Structure:**
    ```
    /lib/features/
    └── favorites/
        ├── api.dart
        ├── data/
        ├── models/
        └── widgets/
    ```

2.  **Define the Model (`models/favorite.dart`):**
    ```dart
    class Favorite {
      final String markerId;
      final DateTime addedAt;
      // ...
    }
    ```

3.  **Create the Logic (`data/favorites_notifier.dart`):**
    ```dart
    import 'package:flutter_riverpod/flutter_riverpod.dart';

    final favoritesNotifierProvider =
        AsyncNotifierProvider<FavoritesNotifier, List<Favorite>>(
      FavoritesNotifier.new,
    );

    class FavoritesNotifier extends AsyncNotifier<List<Favorite>> {
      @override
      Future<List<Favorite>> build() { /* Load from persistence */ }

      Future<void> addFavorite(String markerId) { /* ... */ }
      Future<void> removeFavorite(String markerId) { /* ... */ }
    }
    ```

4.  **Define the Public Contract (`api.dart`):**
    ```dart
    /// The public API for the Favorites feature.
    library;

    export 'data/favorites_notifier.dart'
        show favoritesNotifierProvider, FavoritesNotifier;
    export 'models/favorite.dart' show Favorite;
    ```

5.  **Build the UI (`widgets/favorites_screen.dart`):**
    Create the screen that lists the user's favorites. This widget will `ref.watch(favoritesNotifierProvider)` to get its data.

### How to Use Another Feature

This is where the power of the `api.dart` boundary becomes clear.

**Scenario:** The `locations` feature wants to display a "favorite" icon next to a marker if it's in the user's favorites list.

**The `edit_location_sheet.dart` widget in the `locations` feature would do the following:**

1.  **Import the feature's API:**
    ```dart
    // In lib/features/locations/widgets/edit_location_sheet.dart
    
    import 'package:turbo/features/favorites/api.dart'; // CORRECT!
    
    // DO NOT DO THIS:
    // import 'package:turbo/features/favorites/data/favorites_notifier.dart'; // WRONG!
    ```

2.  **Consume the feature's state:**
    ```dart
    class EditLocationSheet extends ConsumerWidget {
      final Marker location;
      // ...
    
      @override
      Widget build(BuildContext context, WidgetRef ref) {
        // Watch the state from the favorites feature
        final favorites = ref.watch(favoritesNotifierProvider);
        final isFavorite = favorites.value?.any((f) => f.markerId == location.uuid) ?? false;
        
        return Column(
          children: [
            // ... other fields
            IconButton(
              icon: Icon(isFavorite ? Icons.favorite : Icons.favorite_border),
              onPressed: () {
                // Call the logic from the favorites feature
                final notifier = ref.read(favoritesNotifierProvider.notifier);
                if (isFavorite) {
                  notifier.removeFavorite(location.uuid);
                } else {
                  notifier.addFavorite(location.uuid);
                }
              },
            )
          ],
        );
      }
    }
    ```

By importing only `api.dart`, the `locations` feature remains completely decoupled from the *implementation* of the `favorites` feature. We could completely rewrite how favorites are stored and managed, and as long as the `api.dart` contract remains the same, the `locations` feature would not need a single line of code changed.

## 6. Testing Strategy

This architecture makes our code highly testable at two key levels.

### API / Data Layer Testing (Unit/Integration)

We can test the entire business logic of a feature without rendering any UI.

*   **Goal:** Verify that the `LocationRepository` correctly adds a marker when offline.
*   **Location:** `test/features/locations/location_repository_test.dart`
*   **Method:**
    1.  Create a `ProviderContainer`.
    2.  **Override** dependencies from the `core` layer. For example, provide a mock `MarkerDataStore`.
    3.  **Override** the `authStateProvider` (from the `auth` feature's API) to simulate a logged-out user.
    4.  Call the public methods on the `locationRepositoryProvider.notifier`.
    5.  Assert that the state changes as expected and that the mock `MarkerDataStore`'s `insert` method was called.

### UI / Widget Testing (Widget/Golden)

We can test a feature's entire UI by mocking the APIs of the *other features* it depends on.

*   **Goal:** Verify that the `MainMapScreen` correctly shows markers fetched from the `locations` feature.
*   **Location:** `test/features/map_view/main_map_screen_test.dart`
*   **Method:**
    1.  Wrap `MainMapScreen` in a `ProviderScope`.
    2.  **Override** the `locationRepositoryProvider` (from the `locations` feature's API) to return a predefined list of `Marker` objects.
    3.  Pump the widget.
    4.  Assert that the correct number of `MapMarkerWidget` instances are found on the screen.