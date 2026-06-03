/// Public API for the Journey feature — the composition seam that unifies
/// "navigate to a point", "follow a path" and "track a route" into one live
/// state the rest of the app reads and drives.
///
/// This is the only file other features may import. See
/// `lib/context/architecture.context.md` and
/// `docs/architecture/2026-06-composition-overhaul-plan.md` (Phase 2).
library;

export 'models/active_journey.dart' show ActiveJourney, JourneyKind;
export 'data/active_journey_notifier.dart'
    show
        ActiveJourneyNotifier,
        activeJourneyProvider,
        journeyRemainingMetersProvider,
        journeyProgressProvider,
        JourneyProgress;
export 'widgets/journey_path_layer.dart' show JourneyPathLayer;
export 'widgets/active_outing_panel.dart' show ActiveOutingPanel;
