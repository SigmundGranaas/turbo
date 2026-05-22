export const scenarios = {
    smoke: {
        executor: 'constant-vus',
        vus: 5,
        duration: '2m',
        tags: { test_type: 'smoke' },
    },
    load: {
        executor: 'ramping-vus',
        startVUs: 0,
        stages: [
            { duration: '2m', target: 50 },  // Ramp up
            { duration: '5m', target: 50 },  // Stay at 50 users
            { duration: '2m', target: 0 },   // Ramp down
        ],
        tags: { test_type: 'load' },
    },
    stress: {
        executor: 'ramping-arrival-rate',
        startRate: 1,
        timeUnit: '1s',
        preAllocatedVUs: 100,
        maxVUs: 100,
        stages: [
            { duration: '2m', target: 10 },
            { duration: '5m', target: 20 },
            { duration: '2m', target: 30 },
            { duration: '1m', target: 0 },
        ],
        tags: { test_type: 'stress' },
    }
};