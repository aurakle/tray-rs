# List available recipes
default:
    @just --list

# Build all crates
build:
    cargo build -p tray -p tray-menu

# Build tray crate only
build-tray:
    cargo build -p tray

# Build tray-menu crate only
build-menu:
    cargo build -p tray-menu

# Run cargo check on all crates
check:
    cargo check -p tray -p tray-menu

# Clean build artifacts
clean:
    cargo clean

# Run clippy lints
clippy:
    cargo clippy -p tray -p tray-menu -- -D warnings

# Generate documentation
doc:
    cargo doc -p tray -p tray-menu --no-deps

# Run tray simple example
example-tray:
    cargo run -p tray --example simple-tray

# Run iced popup example
example-iced:
    cargo run -p tray --example iced-popup

# Run egui popup example
example-egui:
    cargo run -p tray --example egui-popup

# Run tray-menu simple example (uses default backend on Linux)
example-menu:
    cargo run -p tray-menu --features gtk,qt --example simple-menu

# Run tray-menu simple example with GTK only (Linux)
example-menu-gtk:
    cargo run -p tray-menu --features gtk --example simple-menu

# Run tray-menu simple example with Qt only (Linux)
example-menu-qt:
    cargo run -p tray-menu --features qt --example simple-menu

# Run tray-menu example with left-click trigger
example-menu-left:
    cargo run -p tray-menu --features gtk --example simple-menu -- left

# Run tray-menu example with middle-click trigger
example-menu-middle:
    cargo run -p tray-menu --features gtk --example simple-menu -- middle

# Run tray-menu example with hover trigger
example-menu-enter:
    cargo run -p tray-menu --features gtk --example simple-menu -- enter

# Format code
fmt:
    cargo fmt

# Check formatting without changes
fmt-check:
    cargo fmt -- --check

# Run all tests
test:
    cargo test -p tray
    cargo test -p tray-menu

# Run tray tests only
test-tray:
    cargo test -p tray

# Run tray-menu tests only
test-menu:
    cargo test -p tray-menu
