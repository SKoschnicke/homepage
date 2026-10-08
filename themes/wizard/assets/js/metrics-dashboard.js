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

    function getThemeColors() {
        var style = getComputedStyle(document.documentElement);
        return {
            text: style.getPropertyValue('--color-text-primary').trim() || '#bae6fd',
            muted: style.getPropertyValue('--color-text-muted').trim() || '#818cf8',
            accent: style.getPropertyValue('--color-text-accent').trim() || '#fbbf24',
            link: style.getPropertyValue('--color-link').trim() || '#6ee7b7',
            gridLine: style.getPropertyValue('--color-border-subtle').trim() || '#3730a3'
        };
    }

    function updateChartColors() {
        if (!rpsChart || !latencyChart) return;
        var c = getThemeColors();

        rpsChart.data.datasets[0].borderColor = c.link;
        rpsChart.data.datasets[0].backgroundColor = c.link + '33';
        rpsChart.options.scales.y.ticks.color = c.muted;
        rpsChart.options.scales.y.grid.color = c.gridLine + '44';
        rpsChart.update('none');

        latencyChart.data.datasets[0].backgroundColor = [c.link + 'aa', c.accent + 'aa', '#f87171aa'];
        latencyChart.data.datasets[0].borderColor = [c.link, c.accent, '#f87171'];
        latencyChart.options.scales.y.ticks.color = c.muted;
        latencyChart.options.scales.y.grid.color = c.gridLine + '44';
        latencyChart.options.scales.x.ticks.color = c.muted;
        latencyChart.update('none');
    }

    // Re-apply chart colors when theme changes (toggle or OS-level).
    new MutationObserver(function(mutations) {
        mutations.forEach(function(m) {
            if (m.attributeName === 'data-theme') {
                updateChartColors();
            }
        });
    }).observe(document.documentElement, { attributes: true });
    document.addEventListener('themechange', updateChartColors);

    function initCharts() {
        if (rpsChart && latencyChart) return; // Already initialized

        var colors = getThemeColors();
        var rpsCtx = document.getElementById('rps-chart').getContext('2d');
        var latencyCtx = document.getElementById('latency-chart').getContext('2d');

        rpsData = {
            labels: [],
            datasets: [{
                label: 'req/s',
                data: [],
                borderColor: colors.link,
                backgroundColor: colors.link + '33',
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
                        ticks: { color: colors.muted },
                        grid: { color: colors.gridLine + '44' }
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
                        colors.link + 'aa',
                        colors.accent + 'aa',
                        '#f87171aa'
                    ],
                    borderColor: [
                        colors.link,
                        colors.accent,
                        '#f87171'
                    ],
                    borderWidth: 1
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
                        ticks: { color: colors.muted },
                        grid: { color: colors.gridLine + '44' }
                    },
                    x: {
                        ticks: { color: colors.muted },
                        grid: { display: false }
                    }
                }
            }
        });
    }

    function initDashboard() {
        const container = document.getElementById('metrics-dashboard');
        if (!container) return;
        // CSS off: stay hidden and skip the WebSocket entirely.
        if (typeof cssActive !== 'function' || !cssActive()) return;
        container.hidden = false;

        // Build the dashboard after the (static, visually hidden) <h2> that
        // footer.html renders. Label/value pairs are <dl>s, the toggle is a
        // real disclosure button, and the expanded panel uses the `hidden`
        // attribute so its state doesn't depend on CSS.
        container.insertAdjacentHTML('beforeend', `
            <div class="metrics-compact">
                <dl class="metrics-compact-list">
                    <div class="metric-compact-item">
                        <dt class="metric-compact-label">Req/s:</dt>
                        <dd class="metric-compact-value" id="compact-rps">--</dd>
                    </div>
                    <div class="metric-compact-item">
                        <dt class="metric-compact-label">Latency:</dt>
                        <dd class="metric-compact-value" id="compact-latency">--μs</dd>
                    </div>
                    <div class="metric-compact-item">
                        <dt class="metric-compact-label">Viewers:</dt>
                        <dd class="metric-compact-value" id="compact-viewers">--</dd>
                    </div>
                </dl>
                <button type="button" class="metrics-toggle" id="metrics-toggle"
                        aria-expanded="false" aria-controls="metrics-expanded"></button>
            </div>
            <div class="metrics-grid" id="metrics-expanded" hidden>
                <div class="metric-card">
                    <h3>Requests/Second</h3>
                    <canvas id="rps-chart" role="img" aria-label="Requests per second over the last minute"></canvas>
                    <p class="metric-value" id="rps-value">--</p>
                </div>
                <div class="metric-card">
                    <h3>Response Latency (μs)</h3>
                    <canvas id="latency-chart" role="img" aria-label="Response latency percentiles in microseconds"></canvas>
                    <dl class="metric-labels">
                        <div><dt>p50:</dt> <dd id="p50-value">--</dd></div>
                        <div><dt>p95:</dt> <dd id="p95-value">--</dd></div>
                        <div><dt>p99:</dt> <dd id="p99-value">--</dd></div>
                    </dl>
                </div>
                <div class="metric-card">
                    <h3>Server Stats</h3>
                    <dl class="metric-stats">
                        <div class="metric-stat">
                            <dt class="stat-label">Dashboard Viewers:</dt>
                            <dd class="stat-value" id="viewers-value">--</dd>
                        </div>
                        <div class="metric-stat">
                            <dt class="stat-label">Uptime:</dt>
                            <dd class="stat-value" id="uptime-value">--</dd>
                        </div>
                        <div class="metric-stat">
                            <dt class="stat-label">Total Requests:</dt>
                            <dd class="stat-value" id="total-requests-value">--</dd>
                        </div>
                    </dl>
                    <p class="connection-status" id="ws-status" role="status">Connecting...</p>
                </div>
            </div>
        `);

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

        // Keep the panel, the button's ARIA state and its label in sync. The
        // arrow is decoration; screen readers get the state from aria-expanded.
        function renderToggle(expanded) {
            expandedView.hidden = !expanded;
            toggleBtn.setAttribute('aria-expanded', String(expanded));
            toggleBtn.innerHTML = expanded
                ? 'Show less <span aria-hidden="true">▲</span>'
                : 'Show more <span aria-hidden="true">▼</span>';
        }

        function expand() {
            // Load Chart.js if not already loaded, then show the view
            loadChartJs().then(() => {
                initCharts();
                renderToggle(true);
            }).catch(err => {
                console.error('Failed to load Chart.js:', err);
            });
        }

        renderToggle(false);
        // If previously expanded, load Chart.js and initialize immediately
        if (isExpanded) expand();

        toggleBtn.addEventListener('click', function() {
            isExpanded = !isExpanded;
            localStorage.setItem('metricsExpanded', isExpanded);
            if (isExpanded) {
                expand();
            } else {
                renderToggle(false);
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

        // Back/forward cache: an open WebSocket makes the page ineligible, so
        // close it when the page is hidden and reconnect if it's restored.
        window.addEventListener('pagehide', function() {
            if (reconnectTimeout) clearTimeout(reconnectTimeout);
            reconnectTimeout = null;
            if (ws) {
                // Detach first: onclose would otherwise schedule a reconnect
                ws.onopen = ws.onmessage = ws.onerror = ws.onclose = null;
                ws.close();
                ws = null;
            }
        });

        window.addEventListener('pageshow', function(event) {
            if (event.persisted && !ws) connect();
        });

        // Start connection
        connect();
    }
})();
