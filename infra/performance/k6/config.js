export const config = {
    AUTH_URL: __ENV.AUTH_URL || 'http://turboapi.local/auth',
    GEO_URL: __ENV.GEO_URL || 'http://turboapi.local/geo',
    ACTIVITY_URL: __ENV.ACTIVITY_URL || 'http://turboapi.local/activity',
    TEST_USER_COUNT: parseInt(__ENV.TEST_USER_COUNT || '10'),

    // VU Configuration Options
    VUS: parseInt(__ENV.VUS || '10'),
    VU_ITERATIONS: parseInt(__ENV.VU_ITERATIONS || '100'),
    MAX_VUS: parseInt(__ENV.MAX_VUS || '50'),
    DURATION: __ENV.DURATION || '5m',
    RAMP_UP: __ENV.RAMP_UP || '30s',
    RAMP_DOWN: __ENV.RAMP_DOWN || '30s',
    SCENARIO: __ENV.SCENARIO || 'smoke' // Options: smoke, load, stress, custom
};