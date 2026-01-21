// Fine-tune pixel art scaling based on device pixel ratio
// CSS media queries provide fallback; this gives precise values
(function() {
    const PHYSICAL_PIXELS_TARGET = 2;

    function updateArtPixel() {
        const dpr = window.devicePixelRatio || 1;
        const artPixel = PHYSICAL_PIXELS_TARGET / dpr;
        document.documentElement.style.setProperty('--art-pixel', artPixel + 'px');
    }

    // Set immediately
    updateArtPixel();

    // Update if DPR changes (e.g., moving window between displays)
    if (window.matchMedia) {
        window.matchMedia('(resolution: 1dppx)').addEventListener('change', updateArtPixel);
    }
})();
