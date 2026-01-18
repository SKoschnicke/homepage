(function() {
    'use strict';

    let chartJsLoaded = false;
    let chartJsLoading = false;
    let rpsChart = null;
    let latencyChart = null;
    let rpsData = null;

    // Wait for DOM to load
    if (document.readyState === 'loading') {
        document.addEventListener('DOMContentLoaded', initDashboard);
    } else {
        initDashboard();
    }

    function loadChartJs() {
        return new Promise((resolve, reject) => {
            if (chartJsLoaded) {
                resolve();
                return;
            }

            if (chartJsLoading) {
                // Already loading, wait for it
                const checkInterval = setInterval(() => {
                    if (chartJsLoaded) {
                        clearInterval(checkInterval);
                        resolve();
                    }
                }, 50);
                return;
            }

            chartJsLoading = true;
            const script = document.createElement('script');
            script.src = window.CHART_JS_URL;
            script.integrity = window.CHART_JS_INTEGRITY;
            script.crossOrigin = 'anonymous';
            script.onload = () => {
                chartJsLoaded = true;
                chartJsLoading = false;
                resolve();
            };
            script.onerror = () => {
                chartJsLoading = false;
                reject(new Error('Failed to load Chart.js'));
            };
            document.head.appendChild(script);
        });
    }

    function initCharts() {
        if (rpsChart && latencyChart) return; // Already initialized

        const rpsCtx = document.getElementById('rps-chart').getContext('2d');
        const latencyCtx = document.getElementById('latency-chart').getContext('2d');

        rpsData = {
            labels: [],
            datasets: [{
                label: 'req/s',
                data: [],
                borderColor: 'rgb(75, 192, 192)',
                backgroundColor: 'rgba(75, 192, 192, 0.2)',
                tension: 0.4,
                fill: true
            }]
        };

        rpsChart = new Chart(rpsCtx, {
            type: 'line',
            data: rpsData,
            options: {
                responsive: true,
                maintainAspectRatio: false,
                plugins: {
                    legend: { display: false }
                },
                scales: {
                    y: {
                        beginAtZero: true,
                        ticks: { color: 'inherit' }
                    },
                    x: {
                        display: false
                    }
                }
            }
        });

        latencyChart = new Chart(latencyCtx, {
            type: 'bar',
            data: {
                labels: ['p50', 'p95', 'p99'],
                datasets: [{
                    label: 'μs',
                    data: [0, 0, 0],
                    backgroundColor: [
                        'rgba(54, 162, 235, 0.6)',
                        'rgba(255, 206, 86, 0.6)',
                        'rgba(255, 99, 132, 0.6)'
                    ]
                }]
            },
            options: {
                responsive: true,
                maintainAspectRatio: false,
                plugins: {
                    legend: { display: false }
                },
                scales: {
                    y: {
                        beginAtZero: true,
                        ticks: { color: 'inherit' }
                    },
                    x: {
                        ticks: { color: 'inherit' }
                    }
                }
            }
        });
    }

    function initDashboard() {
        const container = document.getElementById('metrics-dashboard');
        if (!container) return;

        // Create dashboard HTML structure
        container.innerHTML = `
            <div class="metrics-compact">
                <span class="metric-compact-item">
                    <span class="metric-compact-label">Req/s:</span>
                    <span class="metric-compact-value" id="compact-rps">--</span>
                </span>
                <span class="metric-compact-item">
                    <span class="metric-compact-label">Latency:</span>
                    <span class="metric-compact-value" id="compact-latency">--μs</span>
                </span>
                <span class="metric-compact-item">
                    <span class="metric-compact-label">Viewers:</span>
                    <span class="metric-compact-value" id="compact-viewers">--</span>
                </span>
                <a href="#" class="metrics-toggle" id="metrics-toggle">Show more ▼</a>
            </div>
            <div class="metrics-grid" id="metrics-expanded" style="display: none;">
                <div class="metric-card">
                    <h3>Requests/Second</h3>
                    <canvas id="rps-chart"></canvas>
                    <div class="metric-value" id="rps-value">--</div>
                </div>
                <div class="metric-card">
                    <h3>Response Latency (μs)</h3>
                    <canvas id="latency-chart"></canvas>
                    <div class="metric-labels">
                        <span>p50: <span id="p50-value">--</span></span>
                        <span>p95: <span id="p95-value">--</span></span>
                        <span>p99: <span id="p99-value">--</span></span>
                    </div>
                </div>
                <div class="metric-card">
                    <h3>Server Stats</h3>
                    <div class="metric-stat">
                        <span class="stat-label">Dashboard Viewers:</span>
                        <span class="stat-value" id="viewers-value">--</span>
                    </div>
                    <div class="metric-stat">
                        <span class="stat-label">Uptime:</span>
                        <span class="stat-value" id="uptime-value">--</span>
                    </div>
                    <div class="metric-stat">
                        <span class="stat-label">Total Requests:</span>
                        <span class="stat-value" id="total-requests-value">--</span>
                    </div>
                    <div class="connection-status" id="ws-status">Connecting...</div>
                </div>
            </div>
        `;

        // WebSocket connection
        const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
        const wsUrl = `${protocol}//${window.location.host}/__metrics__/ws`;
        let ws = null;
        let reconnectTimeout = null;

        function connect() {
            try {
                ws = new WebSocket(wsUrl);

                ws.onopen = function() {
                    console.log('Metrics WebSocket connected');
                    document.getElementById('ws-status').textContent = 'Connected';
                    document.getElementById('ws-status').className = 'connection-status connected';
                };

                ws.onmessage = function(event) {
                    try {
                        const metrics = JSON.parse(event.data);
                        updateDashboard(metrics);
                    } catch (e) {
                        console.error('Failed to parse metrics:', e);
                    }
                };

                ws.onerror = function(error) {
                    console.error('WebSocket error:', error);
                    document.getElementById('ws-status').textContent = 'Error';
                    document.getElementById('ws-status').className = 'connection-status error';
                };

                ws.onclose = function() {
                    console.log('WebSocket closed, reconnecting in 5s...');
                    document.getElementById('ws-status').textContent = 'Reconnecting...';
                    document.getElementById('ws-status').className = 'connection-status reconnecting';

                    if (reconnectTimeout) clearTimeout(reconnectTimeout);
                    reconnectTimeout = setTimeout(connect, 5000);
                };
            } catch (e) {
                console.error('Failed to create WebSocket:', e);
                if (reconnectTimeout) clearTimeout(reconnectTimeout);
                reconnectTimeout = setTimeout(connect, 5000);
            }
        }

        // Toggle functionality
        const toggleBtn = document.getElementById('metrics-toggle');
        const expandedView = document.getElementById('metrics-expanded');
        let isExpanded = localStorage.getItem('metricsExpanded') === 'true';

        // If previously expanded, load Chart.js and initialize immediately
        if (isExpanded) {
            loadChartJs().then(() => {
                initCharts();
                expandedView.style.display = 'grid';
                toggleBtn.textContent = 'Show less ▲';
            }).catch(err => {
                console.error('Failed to load Chart.js:', err);
            });
        }

        toggleBtn.addEventListener('click', function(e) {
            e.preventDefault();
            isExpanded = !isExpanded;
            localStorage.setItem('metricsExpanded', isExpanded);

            if (isExpanded) {
                // Load Chart.js if not already loaded, then show the view
                loadChartJs().then(() => {
                    initCharts();
                    expandedView.style.display = 'grid';
                    toggleBtn.textContent = 'Show less ▲';
                }).catch(err => {
                    console.error('Failed to load Chart.js:', err);
                });
            } else {
                expandedView.style.display = 'none';
                toggleBtn.textContent = 'Show more ▼';
            }
        });

        function updateDashboard(metrics) {
            // Update compact view (always visible)
            document.getElementById('compact-rps').textContent =
                metrics.requests_per_sec.toFixed(1);
            document.getElementById('compact-latency').textContent =
                metrics.p50_micros.toLocaleString() + 'μs';
            document.getElementById('compact-viewers').textContent =
                metrics.websocket_clients;

            // Only update expanded view if charts are initialized
            if (!rpsChart || !latencyChart) return;

            // Update expanded view (charts and detailed stats)
            const now = new Date().toLocaleTimeString();
            rpsData.labels.push(now);
            rpsData.datasets[0].data.push(metrics.requests_per_sec);

            // Keep only last 60 data points
            if (rpsData.labels.length > 60) {
                rpsData.labels.shift();
                rpsData.datasets[0].data.shift();
            }
            rpsChart.update('none');

            // Update RPS value display
            document.getElementById('rps-value').textContent =
                metrics.requests_per_sec.toFixed(1);

            // Update latency chart
            latencyChart.data.datasets[0].data = [
                metrics.p50_micros,
                metrics.p95_micros,
                metrics.p99_micros
            ];
            latencyChart.update('none');

            // Update latency values
            document.getElementById('p50-value').textContent =
                metrics.p50_micros.toLocaleString();
            document.getElementById('p95-value').textContent =
                metrics.p95_micros.toLocaleString();
            document.getElementById('p99-value').textContent =
                metrics.p99_micros.toLocaleString();

            // Update stats
            document.getElementById('viewers-value').textContent =
                metrics.websocket_clients;
            document.getElementById('uptime-value').textContent =
                formatUptime(metrics.uptime_secs);
            document.getElementById('total-requests-value').textContent =
                metrics.total_requests.toLocaleString();
        }

        function formatUptime(seconds) {
            const days = Math.floor(seconds / 86400);
            const hours = Math.floor((seconds % 86400) / 3600);
            const minutes = Math.floor((seconds % 3600) / 60);
            const secs = seconds % 60;

            if (days > 0) {
                return `${days}d ${hours}h ${minutes}m`;
            } else if (hours > 0) {
                return `${hours}h ${minutes}m ${secs}s`;
            } else if (minutes > 0) {
                return `${minutes}m ${secs}s`;
            } else {
                return `${secs}s`;
            }
        }

        // Start connection
        connect();
    }
})();
