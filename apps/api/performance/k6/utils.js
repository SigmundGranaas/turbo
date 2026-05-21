import { randomString } from 'https://jslib.k6.io/k6-utils/1.2.0/index.js';
import http from 'k6/http';
import { check, sleep } from 'k6';
import { SharedArray } from 'k6/data';
import { config } from './config.js';
import { metrics } from './metrics.js';

// Create test users array
export function createTestUsers(prefix, count) {
    return new SharedArray(`${prefix} users`, function() {
        return Array.from({ length: count }, (_, i) => ({
            email: `${prefix}${i}@example.com`,
            password: 'TestPassword123!'
        }));
    });
}

// Generate random coordinates
export function randomCoordinates() {
    return {
        longitude: Math.random() * 360 - 180,
        latitude: Math.random() * 180 - 90
    };
}

// Generate random extent (bounding box)
export function randomExtent() {
    const minLon = Math.random() * 360 - 180;
    const maxLon = minLon + (Math.random() * 10);
    const minLat = Math.random() * 180 - 90;
    const maxLat = minLat + (Math.random() * 10);

    return { minLon, maxLon, minLat, maxLat };
}

// Generate activity data
export function generateActivityData() {
    return {
        position: randomCoordinates(),
        name: `Test Activity ${randomString(8)}`,
        description: `Description for test activity ${randomString(16)}`,
        icon: `icon-${randomString(4)}`
    };
}