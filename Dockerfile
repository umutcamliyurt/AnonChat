# Use Debian 12 as the base image
FROM debian:12

# Install system dependencies and Rust
RUN apt-get update && apt-get install -y \
    curl \
    build-essential \
    git \
    && curl https://sh.rustup.rs -sSf | sh -s -- -y \
    && . "$HOME/.cargo/env" \
    && rustc --version

# Add Rust to PATH
ENV PATH="/root/.cargo/bin:${PATH}"

# Set the working directory
WORKDIR /app

# Clone the repository
RUN git clone https://github.com/umutcamliyurt/AnonChat.git .

# Build the Rust project in release mode
RUN cargo build --release

# Expose port 80 for the application
EXPOSE 80

# Run the application
CMD ["cargo", "run", "--release"]

