// Theme handling.
//
// No explicit choice: leave data-theme off so CSS media queries drive the scheme,
// which means OS changes take effect live.
// Explicit choice (via toggle): data-theme is set and persisted to sessionStorage,
// overriding the OS preference for the current tab only.

var darkQuery = window.matchMedia('(prefers-color-scheme: dark)');

function effectiveTheme() {
    var attr = document.documentElement.getAttribute('data-theme');
    if (attr) return attr;
    return darkQuery.matches ? 'dark' : 'light';
}

(function() {
    var savedTheme = sessionStorage.getItem('theme');
    if (savedTheme) {
        document.documentElement.setAttribute('data-theme', savedTheme);
    }
})();

function toggleTheme() {
    var newTheme = effectiveTheme() === 'dark' ? 'light' : 'dark';
    document.documentElement.setAttribute('data-theme', newTheme);
    sessionStorage.setItem('theme', newTheme);
    updateThemeIcons(newTheme);
    document.dispatchEvent(new CustomEvent('themechange', { detail: { theme: newTheme } }));
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

darkQuery.addEventListener('change', function() {
    if (sessionStorage.getItem('theme')) return; // user override wins for this session
    var theme = effectiveTheme();
    updateThemeIcons(theme);
    document.dispatchEvent(new CustomEvent('themechange', { detail: { theme: theme } }));
});

document.addEventListener('DOMContentLoaded', function() {
    updateThemeIcons(effectiveTheme());
});
