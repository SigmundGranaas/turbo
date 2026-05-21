import http from 'k6/http';
import { check } from 'k6';
import { config } from './config.js';
import { metrics } from './metrics.js';

// Authenticate a user (login or register if needed)
export function authenticateUser(user) {
    metrics.authAttempts.add(1);

    const loginPayload = JSON.stringify({
        email: user.email,
        password: user.password
    });

    const loginRes = http.post(`${config.AUTH_URL}/api/auth/login`, loginPayload, {
        headers: { 'Content-Type': 'application/json' }
    });

    if (loginRes.status === 200) {
        return { token: JSON.parse(loginRes.body).accessToken };
    }

    // If login fails, try registering
    const registerPayload = JSON.stringify({
        email: user.email,
        password: user.password,
        confirmPassword: user.password
    });

    const registerRes = http.post(`${config.AUTH_URL}/api/auth/register`, registerPayload, {
        headers: { 'Content-Type': 'application/json' }
    });

    if (registerRes.status === 200) {
        // Login after registration
        const loginAfterRegister = http.post(`${config.AUTH_URL}/api/auth/login`, loginPayload, {
            headers: { 'Content-Type': 'application/json' }
        });

        if (loginAfterRegister.status === 200) {
            return { token: JSON.parse(loginAfterRegister.body).accessToken };
        }
    } else {
        metrics.logRequestError('Registration', registerRes);
    }

    metrics.failedLogins.add(1);
    return null;
}

// Setup authenticated sessions
export function setupSessions(testUsers) {
    console.log(`Starting authentication setup for ${testUsers.length} users...`);

    const sessions = testUsers.map((user, index) => {
        console.log(`Authenticating user ${index + 1}/${testUsers.length}: ${user.email}`);
        const session = authenticateUser(user);

        if (session) {
            // Create a properly structured session object
            const formattedSession = {
                user: user,
                token: session.token,
                createdLocations: [],
                createdActivities: {
                    all: [],
                    ready: []
                }
            };

            // Validate the structure after creation
            if (!Array.isArray(formattedSession.createdLocations)) {
                formattedSession.createdLocations = [];
            }

            if (!formattedSession.createdActivities) {
                formattedSession.createdActivities = { all: [], ready: [] };
            }

            if (!Array.isArray(formattedSession.createdActivities.all)) {
                formattedSession.createdActivities.all = [];
            }

            if (!Array.isArray(formattedSession.createdActivities.ready)) {
                formattedSession.createdActivities.ready = [];
            }

            console.log(`User ${user.email} authenticated successfully`);
            return formattedSession;
        }

        console.log(`Failed to authenticate user ${user.email}`);
        return null;
    }).filter(session => session !== null);

    console.log(`Authentication setup complete. ${sessions.length} users authenticated.`);
    return { sessions };
}
