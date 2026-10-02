// Theme handling.
//
// No explicit choice: leave data-theme off so CSS media queries drive the scheme,
// which means OS changes take effect live.
// Explicit choice (via toggle): data-theme is set and persisted to sessionStorage,
// overriding the OS preference for the current tab only.

var darkQuery = window.matchMedia('(prefers-color-scheme: dark)');

// True when the site stylesheet applies (it sets the --css-loaded sentinel on
// :root). JS-only widgets ship with the `hidden` attribute and are revealed
// only if this holds, so with CSS off the page stays plain, readable markup.
// Global on purpose: this file is inlined in <head>, so the deferred widget
// scripts can rely on it.
function cssActive() {
    return getComputedStyle(document.documentElement)
        .getPropertyValue('--css-loaded').trim() !== '';
}

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
    var label = document.querySelector('.nav-toggle-theme-label');
    if (label) {
        label.textContent = label.getAttribute(theme === 'dark' ? 'data-label-light' : 'data-label-dark');
    }
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
    var toggle = document.querySelector('.nav-toggle-theme');
    if (toggle && cssActive()) toggle.hidden = false;
});
