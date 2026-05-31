/// Public façade for the activities_today feature. The shell pulls the
/// Today surface in here without importing per-kind features —
/// recommendation responses come back as kind keys + activity ids, and
/// the screen looks the kind descriptor up through the registry.
library;

export 'data/today_recommendations_api.dart'
    show
        TodayRecommendationsApi,
        todayRecommendationsApiProvider,
        todayRecommendationsProvider;
export 'models/recommendation_item.dart'
    show RecommendationItem, RecommendationsResponse;
export 'models/today_query.dart' show TodayQuery;
export 'widgets/today_card.dart' show TodayCard;
export 'widgets/today_screen.dart' show TodayScreen;
