// Set theme immediately (before paint) to avoid flash
(function() {
    var savedTheme = localStorage.getItem('theme');
    var prefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
    var theme = savedTheme || (prefersDark ? 'dark' : 'light');
    document.documentElement.setAttribute('data-theme', theme);
})();

function toggleTheme() {
    var currentTheme = document.documentElement.getAttribute('data-theme') || 'dark';
    var newTheme = currentTheme === 'dark' ? 'light' : 'dark';
    document.documentElement.setAttribute('data-theme', newTheme);
    localStorage.setItem('theme', newTheme);
    updateThemeIcons(newTheme);
}

function updateThemeIcons(theme) {
    var moon = document.querySelector('.moon');
    var sun = document.querySelector('.sun');
    if (!moon || !sun) return;
    if (theme === 'dark') {
        moon.style.display = 'none';
        sun.style.display = 'inline';
    } else {
        moon.style.display = 'inline';
        sun.style.display = 'none';
    }
}

// Update icons once DOM is ready
document.addEventListener('DOMContentLoaded', function() {
    var theme = document.documentElement.getAttribute('data-theme') || 'dark';
    updateThemeIcons(theme);
});
