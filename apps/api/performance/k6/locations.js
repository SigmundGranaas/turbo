import http from 'k6/http';
import { check } from 'k6';
import { randomCoordinates, randomExtent } from './utils.js';
import { config } from './config.js';
import { metrics } from './metrics.js';

// Create a new location
export function createLocation(requestConfig, session) {
    const coords = randomCoordinates();
    const payload = JSON.stringify({
        longitude: coords.longitude,
        latitude: coords.latitude
    });

    const res = http.post(`${config.GEO_URL}/api/locations`, payload, {
        ...requestConfig,
        tags: { name: 'CreateLocation' }
    });

    if (res.status !== 201) {
        metrics.logRequestError('CreateLocation', res);
    }

    if (check(res, {
        'create location success': (r) => r.status === 201,
        'has location id': (r) => JSON.parse(r.body) !== undefined
    })) {
        metrics.locationCreations.add(1);
        session.createdLocations.push(JSON.parse(res.body));
        return JSON.parse(res.body);
    }

    return null;
}

// Update a location
export function updateLocation(requestConfig, session) {
    if (session.createdLocations.length === 0) {
        return createLocation(requestConfig, session);
    }

    const locationIndex = Math.floor(Math.random() * session.createdLocations.length);
    const locationId = session.createdLocations[locationIndex];
    const coords = randomCoordinates();
    const payload = JSON.stringify({
        longitude: coords.longitude,
        latitude: coords.latitude
    });

    const res = http.put(
        `${config.GEO_URL}/api/locations/${locationId}/position`,
        payload,
        {
            ...requestConfig,
            tags: { name: 'UpdateLocation' }
        }
    );

    if (res.status !== 204) {
        metrics.logRequestError('UpdateLocation', res);
    }

    if (check(res, {
        'update location success': (r) => r.status === 204
    })) {
        metrics.locationUpdates.add(1);
    }

    return res;
}

// Get a specific location
export function getLocation(requestConfig, session) {
    if (session.createdLocations.length === 0) {
        return createLocation(requestConfig, session);
    }

    const locationIndex = Math.floor(Math.random() * session.createdLocations.length);
    const locationId = session.createdLocations[locationIndex];

    const res = http.get(
        `${config.GEO_URL}/api/locations/${locationId}`,
        {
            ...requestConfig,
            tags: { name: 'GetLocation' }
        }
    );

    if (res.status !== 200) {
        metrics.logRequestError('GetLocation', res);
    }

    check(res, {
        'get location success': (r) => r.status === 200,
        'has valid location data': (r) => {
            const body = JSON.parse(r.body);
            return body.id && typeof body.longitude === 'number' && typeof body.latitude === 'number';
        }
    });

    return res;
}

// Get locations in geographic extent
export function getLocationsInExtent(requestConfig) {
    const { minLon, maxLon, minLat, maxLat } = randomExtent();

    const res = http.get(
        `${config.GEO_URL}/api/locations?minLon=${minLon}&maxLon=${maxLon}&minLat=${minLat}&maxLat=${maxLat}`,
        {
            ...requestConfig,
            tags: { name: 'GetLocationsInExtent' }
        }
    );

    if (res.status !== 200) {
        metrics.logRequestError('GetLocationsInExtent', res);
    }

    check(res, {
        'get locations in extent success': (r) => r.status === 200,
        'has valid locations array': (r) => Array.isArray(JSON.parse(r.body))
    });

    return res;
}

// Delete a location
export function deleteLocation(requestConfig, session) {
    if (session.createdLocations.length === 0) {
        return null;
    }

    const locationId = session.createdLocations.pop();

    const res = http.del(
        `${config.GEO_URL}/api/locations/${locationId}`,
        null,
        {
            ...requestConfig,
            tags: { name: 'DeleteLocation' }
        }
    );

    if (res.status !== 204) {
        metrics.logRequestError('DeleteLocation', res);
    }

    check(res, {
        'delete location success': (r) => r.status === 204
    });

    return res;
}