import { sleep } from 'k6';
import { scenarios } from './scenarios.js';
import { config } from './config.js';
import { createTestUsers } from './utils.js';
import { setupSessions } from './auth.js';
import {
    createLocation,
    updateLocation,
    getLocation,
    getLocationsInExtent,
    deleteLocation
} from './locations.js';

// Create test users
const testUsers = createTestUsers('locationtester', config.TEST_USER_COUNT);

// Define thresholds
export const options = {
    scenarios: scenarios,
    thresholds: {
        'http_req_duration{name:CreateLocation}': ['p(95)<500'],
        'http_req_duration{name:UpdateLocation}': ['p(95)<400'],
        'http_req_duration{name:GetLocation}': ['p(95)<300'],
        'http_req_duration{name:GetLocationsInExtent}': ['p(95)<400'],
        'http_req_duration{name:DeleteLocation}': ['p(95)<400'],
        'http_req_failed': ['rate<0.01'],
    },
};

// Setup authenticated sessions
export function setup() {
    return setupSessions(testUsers);
}

// Default function executed for each VU
export default function(data) {
    if (!data.sessions || data.sessions.length === 0) {
        console.log('No authenticated sessions available');
        return;
    }

    // Get a random session
    const sessionIndex = Math.floor(Math.random() * data.sessions.length);
    const session = data.sessions[sessionIndex];

    const requestConfig = {
        headers: {
            'Content-Type': 'application/json',
            'Authorization': `Bearer ${session.token}`
        }
    };

    // Choose a random operation to perform based on weights
    const rand = Math.random();

    if (rand < 0.3) {
        createLocation(requestConfig, session);
    } else if (rand < 0.5) {
        updateLocation(requestConfig, session);
    } else if (rand < 0.7) {
        getLocation(requestConfig, session);
    } else if (rand < 0.9) {
        getLocationsInExtent(requestConfig);
    } else {
        deleteLocation(requestConfig, session);
    }

    sleep(0.1);
}