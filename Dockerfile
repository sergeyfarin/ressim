# Multi-stage Dockerfile for Ressim Simulator
# Base: Rust with WebAssembly support
# Includes: wasm-pack, Node.js, all dev dependencies

FROM rust:latest as builder

# Set working directory
WORKDIR /app

# Install system dependencies
RUN apt-get update && apt-get install -y \
    curl \
    git \
    build-essential \
    pkg-config \
    python3 \
    && rm -rf /var/lib/apt/lists/*

# Install wasm-pack for WebAssembly compilation
RUN curl https://rustwasm.org/wasm-pack/installer/init.sh -sSf | sh

# Install Node.js LTS (for frontend development)
RUN curl -fsSL https://deb.nodesource.com/setup_20.x | bash - && \
    apt-get install -y nodejs && \
    rm -rf /var/lib/apt/lists/*

# Install Bun package manager (optional, fast alternative to npm)
RUN npm install -g bun

# Install wasm32 target for Rust
RUN rustup target add wasm32-unknown-unknown

# Copy entire project
COPY . /app

# Build Rust WebAssembly module
WORKDIR /app/src/lib/ressim
RUN cargo install wasm-pack
RUN wasm-pack build --target bundler --release

# Return to app root for Node.js build
WORKDIR /app

# Install Node dependencies
RUN npm install

# Production image
FROM node:20-slim

WORKDIR /app

# Install runtime dependencies only
RUN apt-get update && apt-get install -y \
    curl \
    git \
    && rm -rf /var/lib/apt/lists/*

# Copy built artifacts from builder
COPY --from=builder /app/node_modules /app/node_modules
COPY --from=builder /app/src/lib/ressim/pkg /app/src/lib/ressim/pkg
COPY --from=builder /app /app

# Expose port for development server
EXPOSE 5173

# Set environment variables
ENV NODE_ENV=development

# Default command: start Vite dev server
CMD ["npm", "run", "dev", "--", "--host"]
