export function startComputerDraw(canvas: HTMLCanvasElement): { stop: () => void } {
    const ctx = canvas.getContext('2d');
    if (!ctx) {
        throw new Error('Canvas rendering context not available.');
    }

    let animationFrameId: number;
    let isRunning = true;
    let elapsedTime = 0; // Timer to track elapsed time in seconds
    const particles: { x: number; y: number; vx: number; vy: number; radius: number; color: string }[] = [];
    const gravity = 0.05; // Gravity factor
    const bounce = 0.9; // Bounce factor
    const bigBallBaseRadius = 50; // Base radius of the big ball
    let bigBallActive = false;
    let bigBallX = 0;
    let bigBallY = 0;
    let bigBallRadius = bigBallBaseRadius;
    let bigBallColor = 'green'; // 'green' or 'red' to determine attraction or repulsion
    let bigBallTimer: number | undefined;

    // Generate particles with random size and color
    for (let i = 0; i < 500; i++) {
        const radius = 2 + Math.random() * 3; // Random radius between 2 and 5
        const color = `hsl(${Math.random() * 360}, 80%, 60%)`; // Random hue for color

        particles.push({
            x: Math.random() * canvas.width,
            y: Math.random() * canvas.height,
            vx: (Math.random() - 0.5) * 4, // Random velocity X
            vy: (Math.random() - 0.5) * 4, // Random velocity Y
            radius,
            color,
        });
    }

    // Function to draw particles and timer
    function draw() {
        if (!isRunning) return;

        const width = canvas.width;
        const height = canvas.height;

        // Clear the canvas with a white background
        ctx.fillStyle = 'white';
        ctx.fillRect(0, 0, width, height);

        // Draw timer on the top-left corner
        ctx.fillStyle = 'black';
        ctx.font = '20px Arial';
        ctx.fillText(`Timer: ${Math.floor(elapsedTime)}s`, 10, 30);

        for (const particle of particles) {
            // Update particle position based on velocity
            particle.vy += gravity; // Apply gravity

            // Attract or repel particles based on the big ball's color
            if (bigBallActive) {
                const forceConstant = 1; // Strength of the attraction/repulsion
                const dx = bigBallX - particle.x;
                const dy = bigBallY - particle.y;
                const distance = Math.sqrt(dx * dx + dy * dy);

                if (distance > 0) {
                    // The size of the big ball affects the strength of the attraction/repulsion
                    const forceMultiplier = bigBallRadius / bigBallBaseRadius;

                    if (bigBallColor === 'green') {
                        // Attract the particle towards the big ball
                        const attractionForce = forceConstant * forceMultiplier; // Attraction force
                        particle.vx += (dx / distance) * attractionForce;
                        particle.vy += (dy / distance) * attractionForce;
                    } else if (bigBallColor === 'red') {
                        // Push the particle away from the big ball
                        const repulsionForce = forceConstant * forceMultiplier; // Repulsion force
                        particle.vx -= (dx / distance) * repulsionForce;
                        particle.vy -= (dy / distance) * repulsionForce;
                    }
                }
            }

            particle.x += particle.vx;
            particle.y += particle.vy;

            // Bounce off the edges
            if (particle.x + particle.radius > width || particle.x - particle.radius < 0) {
                particle.vx = -particle.vx * bounce;
            }
            if (particle.y + particle.radius > height || particle.y - particle.radius < 0) {
                particle.vy = -particle.vy * bounce;
            }

            // Ensure particles stay within the bounds
            particle.x = Math.max(particle.radius, Math.min(particle.x, width - particle.radius));
            particle.y = Math.max(particle.radius, Math.min(particle.y, height - particle.radius));

            // Draw particle
            ctx.beginPath();
            ctx.arc(particle.x, particle.y, particle.radius, 0, 2 * Math.PI);
            ctx.fillStyle = particle.color;
            ctx.fill();
        }

        // If big ball is active, draw it
        if (bigBallActive) {
            ctx.beginPath();
            ctx.arc(bigBallX, bigBallY, bigBallRadius, 0, 2 * Math.PI);
            ctx.fillStyle = bigBallColor === 'green' ? 'rgba(0, 255, 0, 0.5)' : 'rgba(255, 0, 0, 0.5)';
            ctx.fill();
        }

        // Request the next frame
        animationFrameId = requestAnimationFrame(draw);
    }

    // Function to create a big ball that attracts or repels particles
    function spawnBigBall() {
        if (!isRunning) return;

        bigBallX = Math.random() * canvas.width;
        bigBallY = Math.random() * canvas.height;

        // Randomly decide whether the big ball is green (attracts) or red (repels)
        bigBallColor = Math.random() > 0.5 ? 'green' : 'red';

        // Randomize the big ball size
        bigBallRadius = bigBallBaseRadius + Math.random() * 30;

        bigBallActive = true;

        // Remove the big ball after 2 seconds
        bigBallTimer = window.setTimeout(() => {
            bigBallActive = false;
        }, 2000);
    }

    // Start spawning a big ball every 5 seconds
    const bigBallInterval = setInterval(spawnBigBall, 5000);

    // Function to update the elapsed time
    const updateElapsedTime = setInterval(() => {
        elapsedTime += 1; // Increment the elapsed time every second
    }, 1000);

    // Start the drawing loop
    draw();

    // Return a handle to stop the animation
    return {
        stop: () => {
            isRunning = false;
            cancelAnimationFrame(animationFrameId);
            clearInterval(bigBallInterval);
            clearInterval(updateElapsedTime);
            if (bigBallTimer) {
                clearTimeout(bigBallTimer);
            }
        },
    };
}
