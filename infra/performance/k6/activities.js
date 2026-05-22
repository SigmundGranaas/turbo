import http from 'k6/http';
import { check, sleep } from 'k6';
import { generateActivityData } from './utils.js';
import { config } from './config.js';
import { metrics } from './metrics.js';
import { randomString } from 'https://jslib.k6.io/k6-utils/1.2.0/index.js';

// Create a new activity
export function createActivity(requestConfig, session) {
    try {
        // Double-check session is valid
        if (!session) {
            console.log('Session is null or undefined in createActivity');
            metrics.activityErrors.add(1);
            return null;
        }

        // Initialize createdActivities object with structure if it doesn't exist
        if (!session.createdActivities) {
            console.log('Initializing session.createdActivities in createActivity');
            session.createdActivities = {
                all: [],
                ready: []
            };
        }

        // Further validate the structure
        if (!Array.isArray(session.createdActivities.all)) {
            console.log('session.createdActivities.all is not an array, recreating it');
            session.createdActivities.all = [];
        }

        if (!Array.isArray(session.createdActivities.ready)) {
            console.log('session.createdActivities.ready is not an array, recreating it');
            session.createdActivities.ready = [];
        }

        const activityData = generateActivityData();

        const res = http.post(
            `${config.ACTIVITY_URL}/api/activity`,
            JSON.stringify(activityData),
            {
                ...requestConfig,
                tags: { name: 'CreateActivity' }
            }
        );

        metrics.activityLatencies.add(res.timings.duration);

        if (res.status !== 201) {
            metrics.logRequestError('CreateActivity', res);
        }

        // Verify we got a successful response
        if (check(res, {
            'create activity success': (r) => r.status === 201,
            'has activity id': (r) => {
                try {
                    const body = JSON.parse(r.body);
                    return body && body.activityId !== undefined;
                } catch (e) {
                    console.log(`Error parsing response body: ${e.message}`);
                    return false;
                }
            }
        })) {
            try {
                const body = JSON.parse(res.body);
                const activityId = body.activityId;

                // Final verification before push
                if (Array.isArray(session.createdActivities.all)) {
                    session.createdActivities.all.push(activityId);
                    metrics.activeActivities.add(1);
                    return activityId;
                } else {
                    console.log('session.createdActivities.all is still not an array after initialization');
                    metrics.activityErrors.add(1);
                    return null;
                }
            } catch (e) {
                console.log(`Error extracting activity ID: ${e.message}`);
                metrics.activityErrors.add(1);
                return null;
            }
        } else {
            metrics.activityErrors.add(1);
            return null;
        }
    } catch (error) {
        console.log(`Error in createActivity: ${error.message}`);
        metrics.activityErrors.add(1);
        return null;
    }
}

// Get an activity
export function getActivity(requestConfig, session) {
    try {
        // Double-check session is valid
        if (!session) {
            console.log('Session is null or undefined in getActivity');
            metrics.activityErrors.add(1);
            return null;
        }

        // Initialize createdActivities object with structure if it doesn't exist
        if (!session.createdActivities) {
            console.log('Initializing session.createdActivities in getActivity');
            session.createdActivities = {
                all: [],
                ready: []
            };
        }

        // Further validate the structure
        if (!Array.isArray(session.createdActivities.all)) {
            console.log('session.createdActivities.all is not an array, recreating it');
            session.createdActivities.all = [];
        }

        if (!Array.isArray(session.createdActivities.ready)) {
            console.log('session.createdActivities.ready is not an array, recreating it');
            session.createdActivities.ready = [];
        }

        // If no activities created yet, create one
        if (session.createdActivities.all.length === 0) {
            return createActivity(requestConfig, session);
        }

        // Get a random activity ID
        const activityIndex = Math.floor(Math.random() * session.createdActivities.all.length);
        const activityId = session.createdActivities.all[activityIndex];

        let attempts = 0;
        const maxAttempts = 3;

        while (attempts < maxAttempts) {
            const res = http.get(
                `${config.ACTIVITY_URL}/api/activity/${activityId}`,
                {
                    ...requestConfig,
                    tags: { name: 'GetActivity' },
                    validateResponseStatus: false
                }
            );

            metrics.activityLatencies.add(res.timings.duration);
            attempts++;

            if (res.status !== 200 && res.status !== 404) {
                metrics.logRequestError('GetActivity', res);
            }

            if (res.status === 200) {
                // Success - mark as ready
                if (!session.createdActivities.ready.includes(activityId)) {
                    session.createdActivities.ready.push(activityId);
                }
                return res;
            } else if (res.status === 404 && attempts < maxAttempts) {
                // Activity not ready - wait and retry
                sleep(1); // Wait 1 second before retry
            } else {
                // Other error or max attempts reached
                metrics.activityErrors.add(1);
                break;
            }
        }

        return null;
    } catch (error) {
        console.log(`Error in getActivity: ${error.message}`);
        metrics.activityErrors.add(1);
        return null;
    }
}

// Edit an activity
export function editActivity(requestConfig, session) {
    try {
        // Double-check session is valid
        if (!session) {
            console.log('Session is null or undefined in editActivity');
            metrics.activityErrors.add(1);
            return null;
        }

        // Initialize createdActivities object with structure if it doesn't exist
        if (!session.createdActivities) {
            console.log('Initializing session.createdActivities in editActivity');
            session.createdActivities = {
                all: [],
                ready: []
            };
        }

        // Further validate the structure
        if (!Array.isArray(session.createdActivities.all)) {
            console.log('session.createdActivities.all is not an array, recreating it');
            session.createdActivities.all = [];
        }

        if (!Array.isArray(session.createdActivities.ready)) {
            console.log('session.createdActivities.ready is not an array, recreating it');
            session.createdActivities.ready = [];
        }

        if (session.createdActivities.ready.length === 0) {
            // If no ready IDs, try to get one
            getActivity(requestConfig, session);
            return null;
        }

        const activityIndex = Math.floor(Math.random() * session.createdActivities.ready.length);
        const activityId = session.createdActivities.ready[activityIndex];

        const updateData = {
            name: `Updated Activity ${randomString(8)}`,
            description: `Updated description ${randomString(16)}`,
            icon: `icon-updated-${randomString(4)}`
        };

        const res = http.patch(
            `${config.ACTIVITY_URL}/api/activity/${activityId}`,
            JSON.stringify(updateData),
            {
                ...requestConfig,
                tags: { name: 'EditActivity' }
            }
        );

        if (res.status !== 200) {
            metrics.logRequestError('EditActivity', res);
        }

        metrics.activityLatencies.add(res.timings.duration);

        if (!check(res, {
            'edit activity success': (r) => r.status === 200
        })) {
            metrics.activityErrors.add(1);
            // If we get an unauthorized response, remove from ready list
            if (res.status === 401 || res.status === 404) {
                session.createdActivities.ready = session.createdActivities.ready
                    .filter(id => id !== activityId);
                session.createdActivities.all = session.createdActivities.all
                    .filter(id => id !== activityId);
            }
        }

        return res;
    } catch (error) {
        console.log(`Error in editActivity: ${error.message}`);
        metrics.activityErrors.add(1);
        return null;
    }
}

// Delete an activity
export function deleteActivity(requestConfig, session) {
    try {
        // Double-check session is valid
        if (!session) {
            console.log('Session is null or undefined in deleteActivity');
            metrics.activityErrors.add(1);
            return null;
        }

        // Initialize createdActivities object with structure if it doesn't exist
        if (!session.createdActivities) {
            console.log('Initializing session.createdActivities in deleteActivity');
            session.createdActivities = {
                all: [],
                ready: []
            };
        }

        // Further validate the structure
        if (!Array.isArray(session.createdActivities.all)) {
            console.log('session.createdActivities.all is not an array, recreating it');
            session.createdActivities.all = [];
        }

        if (!Array.isArray(session.createdActivities.ready)) {
            console.log('session.createdActivities.ready is not an array, recreating it');
            session.createdActivities.ready = [];
        }

        if (session.createdActivities.ready.length === 0) {
            return null;
        }

        const activityId = session.createdActivities.ready.pop();
        // Remove from main IDs array too
        session.createdActivities.all = session.createdActivities.all
            .filter(id => id !== activityId);

        const res = http.del(
            `${config.ACTIVITY_URL}/api/activity/${activityId}`,
            null,
            {
                ...requestConfig,
                tags: { name: 'DeleteActivity' }
            }
        );

        metrics.activityLatencies.add(res.timings.duration);

        if (res.status !== 200) {
            metrics.logRequestError('DeleteActivity', res);
        }

        if (check(res, {
            'delete activity success': (r) => r.status === 200
        })) {
            metrics.activeActivities.add(-1);
        } else {
            metrics.activityErrors.add(1);
            // If delete failed and it wasn't a 404, put the ID back
            if (res.status !== 404) {
                session.createdActivities.ready.push(activityId);
                session.createdActivities.all.push(activityId);
            }
        }

        return res;
    } catch (error) {
        console.log(`Error in deleteActivity: ${error.message}`);
        metrics.activityErrors.add(1);
        return null;
    }
}