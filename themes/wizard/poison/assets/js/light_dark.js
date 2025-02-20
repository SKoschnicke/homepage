const btn = document.querySelector(".btn-light-dark");
const moon = document.querySelector(".moon");
const sun = document.querySelector(".sun");

// Check for saved theme preference or system preference
const themeFromLS = localStorage.getItem("theme");
const prefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
const currentTheme = themeFromLS || (prefersDark ? "dark" : "light");

// Set initial theme
document.documentElement.setAttribute('data-theme', currentTheme);
// Remove dark-theme class usage
document.body.classList.remove('dark-theme');

// Update icons
if (currentTheme === "dark") {
    moon.style.display = 'none';
    sun.style.display = 'block';
} else {
    moon.style.display = 'block';
    sun.style.display = 'none';
}

btn.addEventListener("click", function () {
    const currentTheme = document.documentElement.getAttribute('data-theme');
    const newTheme = currentTheme === "dark" ? "light" : "dark";
    
    // Update theme
    document.documentElement.setAttribute('data-theme', newTheme);
    
    // Update icons and save preference
    if (newTheme === "dark") {
        moon.style.display = 'none';
        sun.style.display = 'block';
    } else {
        moon.style.display = 'block';
        sun.style.display = 'none';
    }
    localStorage.setItem("theme", newTheme);
});
