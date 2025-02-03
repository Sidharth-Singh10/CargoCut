// load-test.js
import http from 'k6/http';
import { check, sleep } from 'k6';
import { Rate } from 'k6/metrics';
import { randomString } from 'https://jslib.k6.io/k6-utils/1.2.0/index.js';

const errorRate = new Rate('errors');

// Test configuration
export const options = {
  scenarios: {
    // Constant traffic
    constant_load: {
      executor: 'constant-vus',
      vus: 10,
      duration: '1m',
    },
    // Ramping traffic
    ramping_load: {
      executor: 'ramping-vus',
      startVUs: 0,
      stages: [
        { duration: '30s', target: 20 },
        { duration: '1m', target: 20 },
        { duration: '30s', target: 0 },
      ],
      startTime: '1m', // Starts after constant_load
    },
    // Spike test
    spike_test: {
      executor: 'ramping-vus',
      startVUs: 0,
      stages: [
        { duration: '10s', target: 50 }, // Quick ramp-up
        { duration: '20s', target: 50 }, // Stay at peak
        { duration: '10s', target: 0 },  // Quick ramp-down
      ],
      startTime: '3m', // Starts after ramping_load
    },
  },
  thresholds: {
    http_req_duration: ['p(99)<1000'], // 99% of requests should be below 1s
    errors: ['rate<0.1'],              // Error rate should be below 10%
  },
};

// Shared state for created URLs
const createdUrls = new Set();

// Create URL function
function createShortUrl() {
  const payload = JSON.stringify({
    long_url: `https://example.com/${randomString(10)}`,
    months_valid: 1
  });

  const params = {
    headers: {
      'Content-Type': 'application/json',
    },
  };

  const res = http.post('http://localhost:3001/api/urls', payload, params);
  
  check(res, {
    'URL creation successful': (r) => r.status === 200,
  }) || errorRate.add(1);

  if (res.status === 200) {
    const shortCode = JSON.parse(res.body).short_code;
    createdUrls.add(shortCode);
  }

  sleep(1);
}

// Access URL function
function accessShortUrl() {
  if (createdUrls.size === 0) {
    createShortUrl();
    return;
  }

  const shortCodes = Array.from(createdUrls);
  const randomShortCode = shortCodes[Math.floor(Math.random() * shortCodes.length)];
  
  const res = http.get(`http://localhost:3001/${randomShortCode}`);
  
  check(res, {
    'Redirect successful': (r) => r.status === 200,
  }) || errorRate.add(1);

  sleep(0.5);
}

// Main function
export default function () {
  // 30% create new URLs, 70% access existing ones
  if (Math.random() < 0.3) {
    createShortUrl();
  } else {
    accessShortUrl();
  }
}