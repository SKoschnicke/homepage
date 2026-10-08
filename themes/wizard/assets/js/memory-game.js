// Memory Game for the footer
document.addEventListener('DOMContentLoaded', () => {
  const gameContainer = document.getElementById('memory-game');
  const resetButton = document.getElementById('reset-game');
  const statusEl = document.getElementById('memory-game-status');
  const section = gameContainer && gameContainer.closest('.memory-game');
  if (!gameContainer || !section) {
    console.error('Memory game container not found!');
    return;
  }
  // Without the stylesheet the "game" is just a list of emoji answers: keep
  // it hidden and don't build it at all.
  if (typeof cssActive !== 'function' || !cssActive()) return;
  section.hidden = false;

  // Localized labels/announcements, rendered by footer.html from i18n.
  const strings = JSON.parse(section.dataset.strings || '{}');
  const t = (key, vars) => (strings[key] || key)
    .replace(/\{(\w+)\}/g, (m, name) => (vars && name in vars ? vars[name] : m));

  // Symbols to use for the cards (using emoji for simplicity)
  const symbols = ['🚀', '🌟', '🎮', '🎯', '👾', '🕹️'];
  const allSymbols = [...symbols, ...symbols]; // Duplicate for pairs

  // Symbols live here, not in the DOM: a face-down card must not leak its
  // answer to screen readers (or to anyone poking at the markup).
  const cardSymbols = new WeakMap();

  // Matches the flip transition in memory-game.css (0.15s step-end): only
  // blank a card's face once it has turned away.
  const FLIP_MS = 150;

  // Game state
  let hasFlippedCard = false;
  let lockBoard = false;
  let firstCard, secondCard;
  let matchedPairs = 0;
  let score = 1000;
  let gameStarted = false;

  // Create score counter element (inside the game container for overlay)
  const scoreCounter = document.createElement('div');
  scoreCounter.classList.add('score-counter');
  scoreCounter.setAttribute('aria-hidden', 'true'); // announced via statusEl

  function announce(message) {
    if (statusEl) statusEl.textContent = message;
  }

  function cardNumber(card) {
    return Number(card.dataset.index) + 1;
  }

  // Turn a card face up/down, keeping its visible face and its accessible
  // label in sync. The symbol only exists in the DOM while face up.
  function showCard(card) {
    clearTimeout(card._hideTimer);
    const symbol = cardSymbols.get(card);
    card.classList.add('flipped');
    card.querySelector('.front-face').textContent = symbol;
    card.querySelector('.card-label').textContent = t('cardShown', { n: cardNumber(card), symbol });
  }

  function hideCard(card) {
    card.classList.remove('flipped');
    card.querySelector('.card-label').textContent = t('cardHidden', { n: cardNumber(card) });
    card._hideTimer = setTimeout(() => {
      card.querySelector('.front-face').textContent = '';
    }, FLIP_MS);
  }

  function markMatched(card) {
    card.setAttribute('aria-disabled', 'true');
    card.querySelector('.card-label').textContent =
      t('cardMatched', { n: cardNumber(card), symbol: cardSymbols.get(card) });
  }

  // First, show all cards briefly then flip them
  function initialCardReveal() {
    const cards = gameContainer.querySelectorAll('.memory-card');

    // Show all cards for a moment
    cards.forEach(showCard);
    announce(t('started'));

    // Then flip them back
    setTimeout(() => {
      cards.forEach(hideCard);
      // Enable clicking on cards after the initial reveal
      lockBoard = false;
    }, 1500);
  }

  // Initialize the game
  initGame();

  // Add a click event to the game container to start the game on first interaction
  gameContainer.addEventListener('click', startGameOnFirstClick);

  function startGameOnFirstClick(e) {
    if (!gameStarted) {
      gameStarted = true;
      // Add class to hide the "CLICK TO START" text
      gameContainer.classList.add('game-started');
      // Remove this event listener as it's no longer needed
      gameContainer.removeEventListener('click', startGameOnFirstClick);
      // Start the initial card reveal
      initialCardReveal();

      // Prevent the first click from selecting a card
      e.stopPropagation();
      e.preventDefault();
    }
  }

  // Reset button event listener
  if (resetButton) {
    resetButton.addEventListener('click', resetGame);
  }

  // Initialize the game board
  function initGame() {
    // Clear any existing cards
    gameContainer.replaceChildren();
    // Remove game-started class to show "CLICK TO START" again
    gameContainer.classList.remove('game-started');
    announce('');

    // Reset game state
    hasFlippedCard = false;
    lockBoard = true; // Lock the board until first click
    firstCard = null;
    secondCard = null;
    matchedPairs = 0;
    score = 1000;
    gameStarted = false;

    // Remove score counter if it exists in DOM
    if (scoreCounter.parentNode) {
      scoreCounter.parentNode.removeChild(scoreCounter);
    }
    updateScoreCounter();

    // Shuffle the symbols
    const shuffledSymbols = [...allSymbols].sort(() => 0.5 - Math.random());

    // Create the cards: real buttons, so they're focusable and keyboard
    // operable. The faces are decoration; the label carries the meaning.
    shuffledSymbols.forEach((symbol, index) => {
      const card = document.createElement('button');
      card.type = 'button';
      card.classList.add('memory-card');
      card.dataset.index = index;
      cardSymbols.set(card, symbol);

      const frontFace = document.createElement('span');
      frontFace.classList.add('front-face');
      frontFace.setAttribute('aria-hidden', 'true');

      const backFace = document.createElement('span');
      backFace.classList.add('back-face');
      backFace.setAttribute('aria-hidden', 'true');

      const label = document.createElement('span');
      label.classList.add('card-label', 'visually-hidden');
      label.textContent = t('cardHidden', { n: index + 1 });

      card.append(frontFace, backFace, label);

      card.addEventListener('click', flipCard);
      gameContainer.appendChild(card);
    });
  }

  // Reset the game
  function resetGame() {
    // Remove any confetti
    const confettiContainer = document.querySelector('.confetti-container');
    if (confettiContainer) {
      confettiContainer.remove();
    }

    // Re-initialize the game
    initGame();

    // Immediately start the game without requiring activation
    gameStarted = true;
    gameContainer.classList.add('game-started');
    gameContainer.removeEventListener('click', startGameOnFirstClick);
    initialCardReveal();
  }

  // Card flip function
  function flipCard() {
    if (lockBoard) return;
    if (this === firstCard) return;

    showCard(this);

    if (!hasFlippedCard) {
      // First card flipped
      hasFlippedCard = true;
      firstCard = this;
      return;
    }

    // Second card flipped
    secondCard = this;
    checkForMatch();
  }

  // Check if the cards match
  function checkForMatch() {
    const symbol = cardSymbols.get(firstCard);
    const isMatch = symbol === cardSymbols.get(secondCard);

    if (isMatch) {
      matchedPairs++;
      if (matchedPairs === symbols.length) {
        // The win message (with score) replaces the per-pair announcement.
        setTimeout(showFinalScore, 500);
        setTimeout(celebrateWin, 500);
      } else {
        announce(t('match', { symbol }));
      }
      disableCards();
    } else {
      score = Math.max(0, score - 50);
      updateScoreCounter();
      announce(t('miss'));
      unflipCards();
    }
  }

  // Update the score counter display
  function updateScoreCounter() {
    scoreCounter.textContent = `SCORE ${score}`;
  }

  // Show final score overlay on game board
  function showFinalScore() {
    updateScoreCounter();
    scoreCounter.classList.add('final-score');
    // Append to wrapper for absolute positioning over the game
    const wrapper = gameContainer.parentNode;
    wrapper.appendChild(scoreCounter);
    announce(t('won', { score }));
  }

  // Disable matched cards
  function disableCards() {
    firstCard.removeEventListener('click', flipCard);
    secondCard.removeEventListener('click', flipCard);
    markMatched(firstCard);
    markMatched(secondCard);

    resetBoard();
  }

  // Unflip non-matching cards
  function unflipCards() {
    lockBoard = true;

    setTimeout(() => {
      hideCard(firstCard);
      hideCard(secondCard);

      resetBoard();
    }, 1000);
  }

  // Reset board after each turn
  function resetBoard() {
    [hasFlippedCard, lockBoard] = [false, false];
    [firstCard, secondCard] = [null, null];
  }

  // Celebrate with confetti when winning
  function celebrateWin() {
    // Retro 8-bit confetti effect with proper physics
    const confettiCount = 150;
    // Classic NES-inspired palette
    const colors = ['#fc0000', '#00fc00', '#0000fc', '#fcfc00', '#fc00fc', '#00fcfc', '#fcfcfc', '#3AAFB9', '#59C265'];
    // Pixel-art confetti shapes: square, wide rectangle, tall rectangle
    const shapes = [
      { w: 8, h: 8 },   // square
      { w: 12, h: 6 },  // wide
      { w: 6, h: 12 },  // tall
    ];

    const confettiContainer = document.createElement('div');
    confettiContainer.classList.add('confetti-container');
    confettiContainer.setAttribute('aria-hidden', 'true');
    document.body.appendChild(confettiContainer);

    const confettiPieces = [];

    for (let i = 0; i < confettiCount; i++) {
      const confetti = document.createElement('div');
      confetti.classList.add('confetti');

      const shape = shapes[Math.floor(Math.random() * shapes.length)];
      confetti.style.width = shape.w + 'px';
      confetti.style.height = shape.h + 'px';
      confetti.style.backgroundColor = colors[Math.floor(Math.random() * colors.length)];

      // Initial position - spread across top
      const startX = Math.random() * window.innerWidth;
      confetti.style.left = startX + 'px';
      confetti.style.top = '-20px';

      confettiContainer.appendChild(confetti);

      // Physics properties for each piece
      confettiPieces.push({
        el: confetti,
        x: startX,
        y: -20 - Math.random() * 100, // stagger start
        vx: (Math.random() - 0.5) * 3, // horizontal drift
        vy: Math.random() * 2 + 1,     // fall speed
        rotateX: Math.random() * 360,
        rotateY: Math.random() * 360,
        rotateZ: Math.random() * 360,
        spinX: (Math.random() - 0.5) * 15,  // rotation speed
        spinY: (Math.random() - 0.5) * 15,
        spinZ: (Math.random() - 0.5) * 10,
        wobblePhase: Math.random() * Math.PI * 2,
        wobbleSpeed: Math.random() * 0.1 + 0.05,
        wobbleAmount: Math.random() * 2 + 1,
      });
    }

    // Animate with requestAnimationFrame for smooth 8-bit tumbling
    let frame = 0;
    const maxFrames = 600; // ~10 seconds at 60fps

    function animateConfetti() {
      frame++;
      let allDone = true;

      confettiPieces.forEach(piece => {
        // Update physics
        piece.wobblePhase += piece.wobbleSpeed;
        piece.x += piece.vx + Math.sin(piece.wobblePhase) * piece.wobbleAmount;
        piece.y += piece.vy;
        piece.vy += 0.03; // gravity

        // Air resistance on horizontal movement
        piece.vx *= 0.99;

        // Tumbling rotation
        piece.rotateX += piece.spinX;
        piece.rotateY += piece.spinY;
        piece.rotateZ += piece.spinZ;

        // Apply transform - using steps for that chunky pixel feel
        const stepX = Math.round(piece.x);
        const stepY = Math.round(piece.y);
        const stepRX = Math.round(piece.rotateX / 15) * 15; // snap to 15-degree increments
        const stepRY = Math.round(piece.rotateY / 15) * 15;
        const stepRZ = Math.round(piece.rotateZ / 15) * 15;

        piece.el.style.transform = `translate(${stepX}px, ${stepY}px) rotateX(${stepRX}deg) rotateY(${stepRY}deg) rotateZ(${stepRZ}deg)`;

        // Fade out near bottom
        if (piece.y > window.innerHeight - 100) {
          piece.el.style.opacity = Math.max(0, 1 - (piece.y - (window.innerHeight - 100)) / 100);
        }

        if (piece.y < window.innerHeight + 50) {
          allDone = false;
        }
      });

      if (!allDone && frame < maxFrames) {
        requestAnimationFrame(animateConfetti);
      } else {
        confettiContainer.remove();
      }
    }

    requestAnimationFrame(animateConfetti);
  }
}); 