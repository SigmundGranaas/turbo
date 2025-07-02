# **Turbo UI: Design & Component Guidelines**

## 1. Core Design Principles

This document specifies the design system and architectural patterns for creating UI components in the Turbo application. All new components must adhere to these principles to ensure a consistent, high-quality, and maintainable user interface.

*   **Visual Hierarchy and Focus:** Components must be designed to guide the user's attention to the most important information and actions. We achieve this through the deliberate use of color, typography, and whitespace, not through excessive decoration. The UI should feel uncluttered and focused.
*   **Adaptive and Responsive Layouts:** Every component must function flawlessly on both mobile and desktop screens. Use `LayoutBuilder` to create distinct layouts when necessary, ensuring an ergonomic and optimized experience for every form factor.
*   **Consistency and Reusability:** Build UIs from our established component library. A user must be able to predict a component's function based on its appearance. Avoid creating one-off styles.

## 2. Widget Architecture

### A. Composition over Inheritance

Build complex widgets by composing simpler ones. This is the standard Flutter pattern and provides maximum flexibility.

**Example:** `PrimaryButton` is a `StatelessWidget` that internally builds and configures a `FilledButton`. It does not extend it.
```dart
class PrimaryButton extends StatelessWidget {
  // ... properties
  
  @override
  Widget build(BuildContext context) {
    // Composition: It *uses* a FilledButton.
    return FilledButton(
      style: /* ... */,
      onPressed: /* ... */,
      child: /* ... */,
    );
  }
}
```

### B. Separating Logic from Layout for Responsiveness

For components with significantly different mobile and desktop UIs (like the `LoginScreen`), use the "Controller/View" pattern:

1.  **The Controller Widget (`login_screen.dart`):**
    *   A `StatefulWidget` that holds state (`TextEditingController`s, `GlobalKey`).
    *   Contains all event handling logic (e.g., `_login()`, `_navigateToRegister()`).
    *   Uses a `LayoutBuilder` to select and render the appropriate view, passing state and logic functions as parameters.

2.  **The View Widgets (`login_view_desktop.dart`, `login_view_mobile.dart`):**
    *   `StatelessWidget`s (`ConsumerWidget`s) that are "dumb." They receive all data and functions via their constructor.
    *   Their sole responsibility is to build the UI for a specific platform.

## 3. Core Design System

All components are built from a consistent application of these Material 3 system elements.

### I. Color

**Rule:** Always use colors from the theme's `ColorScheme` to ensure light/dark mode compatibility and brand consistency. Never use hard-coded colors.

*   **`colorScheme.primary`:** For primary calls-to-action (e.g., `PrimaryButton`, active icons).
*   **`colorScheme.onPrimary`:** For content (text, icons) placed on a `primary` background.
*   **`colorScheme.surface`:** The default background for most components (e.g., cards, sheets).
*   **`colorScheme.surfaceContainer`, `...Low`, `...High`, `...Highest`:** Use these tonal variants to create visual depth and hierarchy between surfaces without relying on heavy shadows.
*   **`colorScheme.outline`:** For all borders and dividers.
*   **`colorScheme.error`:** For error states and validation messages.

### II. Typography

**Rule:** Use `TextTheme` for all text to ensure consistent sizing and weight.

*   **`textTheme.headlineMedium`:** For primary screen titles.
*   **`textTheme.titleLarge`:** For `AppBar` titles and section headers.
*   **`textTheme.bodyLarge` / `bodyMedium`:** For general content.
*   **`textTheme.labelLarge`:** For button text.

### III. Shape and Corner Radii

**Rule:** Use a consistent set of corner radii to create a modern, unified aesthetic.

*   **Large Radius (`28.0px`):** For major containers like `SearchBar`, `BottomSheet`, and `Dialog`.
*   **Medium Radius (`16.0px`):** For in-flow components like `Card`s and text fields.
*   **Full / Stadium Radius (`100.0px`):** For pill-shaped components like `Button`s.

### IV. Surfaces and Elevation

**Rule:** Prioritize tonal color contrast between surfaces to create depth. Use `elevation` sparingly.

*   **In-Flow Components:** Widgets within a layout (e.g., text fields, cards) should have `elevation: 0` or `1`. Their visual separation should come from their `surfaceContainer` color.
*   **Floating Components:** Widgets that float above other content (e.g., `MapControlButtonBase`) can use a small `elevation` (e.g., `4`) to visually lift them from the background.
*   **Modal Surfaces:** Use `colorScheme.scrim` to dim the background behind dialogs and sheets, focusing the user.

### V. Icons

**Rule:** Use the standard Material Icons set (`Icons.*`). Use the outlined variants for a lighter, more modern feel.

*   Interactive icons must be wrapped in `IconButton` to ensure a minimum 48x48dp touch target and provide ripple feedback.
*   Icon color must come from the `ColorScheme` (e.g., `colorScheme.primary`, `colorScheme.onSurfaceVariant`).

### VI. Animation and Motion

**Rule:** Motion must be purposeful, providing feedback and guiding focus without being distracting. Animations should be quick (typically under 300ms).

*   **Transitions:** Favor fade-and-scale transitions (`FadeTransition`, `ScaleTransition`) for new elements appearing on screen.
*   **Curves:** Use `Curves.fastOutSlowIn` for user-initiated animations like map movements to feel responsive and natural. Use `Curves.elasticOut` for micro-interactions like a marker appearing to add a bit of delight.

## 4. The Turbo Component Kit

Build new UIs from these foundational components whenever possible.

#### A. Controls

*   **`PrimaryButton`:** A `FilledButton` for the main call-to-action on a screen.
*   **`SecondaryButton`:** An `OutlinedButton` for less-critical actions.
*   **`TonalButton`:** A `FilledButton.tonal` for medium-emphasis actions that shouldn't compete with the primary button.
*   **`AuthTextField`:** The standard `TextFormField` configuration. It is `filled`, has a `16px` radius, and no underline for a clean surface appearance.

#### B. Surfaces

*   **Sheets & Dialogs:** Must use a `28px` corner radius. On desktop, their content must be constrained with `ConstrainedBox(maxWidth: ...)` for readability.
*   **`MapControlButtonBase`:** The circular `Card`-based widget for all floating map controls (compass, location, etc.). Ensures consistent size, shape, and elevation.

#### C. Brand-Defining Components

*   **Search Bars:** Always use a `28px` radius. The layout adapts between a compact mobile version and a wider desktop version.
*   **`MapMarkerWidget`:** Uses a custom-painted "droplet" shape for a unique brand identity. A hover-triggered `AnimatedOpacity` effect reveals the title, keeping the map uncluttered by default.