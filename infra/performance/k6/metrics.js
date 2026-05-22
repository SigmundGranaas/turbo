import { Counter, Trend } from 'k6/metrics';

export const metrics = {
    // Authentication metrics
    authAttempts: new Counter('auth_attempts'),
    failedLogins: new Counter('failed_logins'),

    // Activity metrics
    activeActivities: new Counter('active_activities'),
    activityLatencies: new Trend('activity_latencies'),
    activityErrors: new Counter('activity_errors'),

    // Location metrics
    locationCreations: new Counter('location_creations'),
    locationUpdates: new Counter('location_updates'),

    // Error metrics
    errorsByType: new Counter('errors_by_type'),
    errorsByStatusCode: new Counter('errors_by_status_code'),

    // Helper function to log errors
    logRequestError: (operation, res) => {
        const errorKey = `${operation}_${res.status}`;
        metrics.errorsByType.add(1, { operation });
        metrics.errorsByStatusCode.add(1, { status: res.status });

        console.log(`${operation} failed:`, {
            status: res.status,
            body: res.body,
            timings: res.timings,
            url: res.url
        });
    }
};