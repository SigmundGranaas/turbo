import { scenarios } from './scenarios.js';
import { sleep } from 'k6';
import { config } from './config.js';

// Import all test modules
import * as authTest from './test-auth.js';
import * as locationsTest from './test-locations.js';

// Combine all thresholds
export const options = {
    scenarios: {
        auth: {
            ...scenarios.smoke,
            exec: 'runAuthTests',
            tags: { service: 'auth', ...scenarios.smoke.tags }
        },
        locations: {
            ...scenarios.smoke,
            exec: 'runLocationTests',
            tags: { service: 'locations', ...scenarios.smoke.tags }
        },
    },
    thresholds: {
        // Combined thresholds from all tests
        ...authTest.options.thresholds,
        ...locationsTest.options.thresholds,
    },
};

// Setup for all tests
export function setup() {
    return {
        auth: authTest.setup(),
        locations: locationsTest.setup(),
    };
}

// Auth test executor
export function runAuthTests(data) {
    authTest.default(data.auth);
}

// Locations test executor
export function runLocationTests(data) {
    locationsTest.default(data.locations);
}
