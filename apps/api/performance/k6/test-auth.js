import { sleep } from 'k6';
import { scenarios } from './scenarios.js';
import { config } from './config.js';
import { createTestUsers } from './utils.js';
import { authenticateUser } from './auth.js';
import http from 'k6/http';
import { check } from 'k6';
import { randomString } from 'https://jslib.k6.io/k6-utils/1.2.0/index.js';

// Create test users
const testUsers = createTestUsers('authtester', config.TEST_USER_COUNT);

// Define thresholds
export const options = {
    scenarios: scenarios,
    thresholds: {
        'http_req_duration{name:RegisterEndpoint}': ['p(95)<500'],
        'http_req_duration{name:LoginEndpoint}': ['p(95)<400'],
        'http_req_failed': ['rate<0.01'],
    },
};

// Setup authenticated sessions
export function setup() {
    const registeredUsers = [];

    testUsers.forEach((user) => {
        // Try logging in first
        const loginPayload = JSON.stringify({
            email: user.email,
            password: user.password
        });

        const loginResponse = http.post(`${config.AUTH_URL}/api/auth/login`, loginPayload, {
            headers: { 'Content-Type': 'application/json' },
        });

        if (loginResponse.status === 200) {
            registeredUsers.push(user);
        } else {
            // User doesn't exist, try registering
            const registerPayload = JSON.stringify({
                email: user.email,
                password: user.password,
                confirmPassword: user.password
            });

            const registerResponse = http.post(`${config.AUTH_URL}/api/auth/register`, registerPayload, {
                headers: { 'Content-Type': 'application/json' },
            });

            if (registerResponse.status === 200) {
                registeredUsers.push(user);
            }
        }

        sleep(0.5);
    });

    return { registeredUsers };
}

// Perform registration operation
function performRegistration() {
    const payload = JSON.stringify({
        email: `user_${randomString(8)}@example.com`,
        password: 'TestPassword123!',
        confirmPassword: 'TestPassword123!'
    });

    const res = http.post(`${config.AUTH_URL}/api/auth/register`, payload, {
        headers: { 'Content-Type': 'application/json' },
        tags: { name: 'RegisterEndpoint' },
    });

    check(res, {
        'register success': (r) => r.status === 200,
        'has tokens': (r) => {
            const body = JSON.parse(r.body);
            return body.accessToken && body.refreshToken;
        },
    });
}

// Perform login operation
function performLogin(registeredUsers) {
    const userIndex = Math.floor(Math.random() * registeredUsers.length);
    const user = registeredUsers[userIndex];

    const payload = JSON.stringify({
        email: user.email,
        password: user.password
    });

    const res = http.post(`${config.AUTH_URL}/api/auth/login`, payload, {
        headers: { 'Content-Type': 'application/json' },
        tags: { name: 'LoginEndpoint' },
    });

    check(res, {
        'login success': (r) => r.status === 200,
        'has valid tokens': (r) => {
            const body = JSON.parse(r.body);
            return body.accessToken && body.refreshToken;
        },
    });
}

// Default function executed for each VU
export default function(data) {
    const rand = Math.random();

    if (rand < 0.4) {
        performRegistration();
    } else if (rand < 0.8 && data.registeredUsers && data.registeredUsers.length > 0) {
        performLogin(data.registeredUsers);
    } else {
        // Fallback to registration if no users are available
        performRegistration();
    }

    sleep(0.1);
}