// Theme handling. Three preferences: 'system', 'light', 'dark'.
//
// 'system': data-theme is left off so CSS media queries drive the scheme,
// which means OS changes take effect live.
// 'light' / 'dark': data-theme is set and persisted to sessionStorage,
// overriding the OS preference for the current tab only.
//
// The toggle's icon is picked by CSS from data-theme, so it is correct on first
// paint; JS only maintains the button's text label.

var darkQuery = window.matchMedia('(prefers-color-scheme: dark)');

function themePreference() {
    return document.documentElement.getAttribute('data-theme') || 'system';
}

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
    var pref = themePreference();
    if (pref !== 'system') return pref;
    return darkQuery.matches ? 'dark' : 'light';
}

(function() {
    var savedTheme = sessionStorage.getItem('theme');
    if (savedTheme === 'light' || savedTheme === 'dark') {
        document.documentElement.setAttribute('data-theme', savedTheme);
    }
})();

function dispatchThemeChange() {
    document.dispatchEvent(new CustomEvent('themechange', { detail: { theme: effectiveTheme() } }));
}

// Cycle system -> opposite of OS -> same as OS -> system, so that the first
// click from 'system' always visibly changes something.
function toggleTheme() {
    var osTheme = darkQuery.matches ? 'dark' : 'light';
    var otherTheme = osTheme === 'dark' ? 'light' : 'dark';
    var pref = themePreference();
    var next = pref === 'system' ? otherTheme : pref === otherTheme ? osTheme : 'system';

    if (next === 'system') {
        document.documentElement.removeAttribute('data-theme');
        sessionStorage.removeItem('theme');
    } else {
        document.documentElement.setAttribute('data-theme', next);
        sessionStorage.setItem('theme', next);
    }
    updateThemeLabel();
    dispatchThemeChange();
}

function updateThemeLabel() {
    var button = document.querySelector('.nav-toggle-theme');
    if (!button) return;
    var name = button.getAttribute('data-label-' + themePreference());
    var text = button.getAttribute('data-label-prefix') + ': ' + name;
    button.querySelector('.theme-label').textContent = text;
    button.title = text;
}

darkQuery.addEventListener('change', function() {
    if (themePreference() !== 'system') return; // explicit choice wins
    dispatchThemeChange();
});

document.addEventListener('DOMContentLoaded', function() {
    updateThemeLabel();
    var toggle = document.querySelector('.nav-toggle-theme');
    if (toggle && cssActive()) toggle.hidden = false;
});
