// Memory Game for the footer
document.addEventListener('DOMContentLoaded', () => {
  const gameContainer = document.getElementById('memory-game');
  const resetButton = document.getElementById('reset-game');
  if (!gameContainer) {
    console.error('Memory game container not found!');
    return;
  }

  console.log('Memory game initialized');
  
  // Symbols to use for the cards (using emoji for simplicity)
  const symbols = ['🚀', '🌟', '🎮', '🎯'];
  const allSymbols = [...symbols, ...symbols]; // Duplicate for pairs
  
  // Game state
  let hasFlippedCard = false;
  let lockBoard = false;
  let firstCard, secondCard;
  let matchedPairs = 0;
  let gameStarted = false;
  
  // First, show all cards briefly then flip them
  function initialCardReveal() {
    const cards = document.querySelectorAll('.memory-card');
    console.log(`Found ${cards.length} cards to reveal`);
    
    // Show all cards for a moment
    cards.forEach(card => card.classList.add('flipped'));
    
    // Then flip them back
    setTimeout(() => {
      cards.forEach(card => card.classList.remove('flipped'));
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
    gameContainer.innerHTML = '';
    
    // Reset game state
    hasFlippedCard = false;
    lockBoard = true; // Lock the board until first click
    firstCard = null;
    secondCard = null;
    matchedPairs = 0;
    gameStarted = false;
    
    // Shuffle the symbols
    const shuffledSymbols = [...allSymbols].sort(() => 0.5 - Math.random());
    
    // Create the cards
    shuffledSymbols.forEach((symbol, index) => {
      const card = document.createElement('div');
      card.classList.add('memory-card');
      card.dataset.symbol = symbol;
      card.setAttribute('data-index', index);
      
      const frontFace = document.createElement('div');
      frontFace.classList.add('front-face');
      frontFace.textContent = symbol;
      
      const backFace = document.createElement('div');
      backFace.classList.add('back-face');
      
      card.appendChild(frontFace);
      card.appendChild(backFace);
      
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
    initialCardReveal();
  }
  
  // Card flip function
  function flipCard(e) {
    if (lockBoard) return;
    if (this === firstCard) return;
    
    this.classList.add('flipped');
    
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
    const isMatch = firstCard.dataset.symbol === secondCard.dataset.symbol;
    
    if (isMatch) {
      disableCards();
      matchedPairs++;
      
      // Check if all pairs are matched
      if (matchedPairs === symbols.length) {
        setTimeout(celebrateWin, 500);
      }
    } else {
      unflipCards();
    }
  }
  
  // Disable matched cards
  function disableCards() {
    firstCard.removeEventListener('click', flipCard);
    secondCard.removeEventListener('click', flipCard);
    
    resetBoard();
  }
  
  // Unflip non-matching cards
  function unflipCards() {
    lockBoard = true;
    
    setTimeout(() => {
      firstCard.classList.remove('flipped');
      secondCard.classList.remove('flipped');
      
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
    // Simple confetti effect
    const confettiCount = 200;
    const colors = ['#ff0000', '#00ff00', '#0000ff', '#ffff00', '#ff00ff', '#00ffff'];
    
    const confettiContainer = document.createElement('div');
    confettiContainer.classList.add('confetti-container');
    document.body.appendChild(confettiContainer);
    
    for (let i = 0; i < confettiCount; i++) {
      const confetti = document.createElement('div');
      confetti.classList.add('confetti');
      confetti.style.backgroundColor = colors[Math.floor(Math.random() * colors.length)];
      confetti.style.left = Math.random() * 100 + 'vw';
      confetti.style.animationDuration = (Math.random() * 3 + 2) + 's';
      confetti.style.animationDelay = Math.random() * 5 + 's';
      confettiContainer.appendChild(confetti);
    }
    
    // Remove confetti after animation
    setTimeout(() => {
      confettiContainer.remove();
    }, 10000);
  }
}); 