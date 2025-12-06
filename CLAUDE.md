# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

ThockFlow is a keyboard training/typing practice web application built with Rust and WebAssembly. It's based on the boilerplate from [implfuture.dev](https://implfuture.dev/blog/rewriting-the-modern-web-in-rust) and uses a modern Rust web stack with server-side rendering (SSR) and hydration.

## Architecture

### Frontend (WASM)
- **Framework**: Yew 0.19 (React-like framework for Rust/WASM)
- **Entry Point**: `src/bin/app.rs` - hydrates the Yew app in the browser
- **Main App**: `src/lib.rs` - defines routing with `yew-router` and components
- **Routes**: Home (`/`) and Typing (`/typing`)
- **Styling**: TailwindCSS (compiled via Bazel, outputs to `static/css/tailwind.css`)

### Backend (Server)
- **Framework**: Axum 0.5 web server
- **Entry Point**: `server/src/main.rs`
- **SSR**: Uses Yew's ServerRenderer to pre-render HTML, then sends it with WASM init scripts
- **Deployment**: Supports both local server and AWS Lambda (via `lambda-web`)
- **Static Files**: Serves from `app_wasm/`, `static/`, and `bundle/dist/`

### Build System
- **Primary**: Bazel with bzlmod enabled (MODULE.bazel)
- **Rust Targets**:
  - WASM target for frontend (`wasm32-unknown-unknown`)
  - Native Linux targets for server (amd64 and arm64)
- **Cross-compilation**: Uses Zig toolchain for hermetic C/C++ cross-compilation
- **Frontend Tooling**: TypeScript/Vite/esbuild for JavaScript bundle processing

### Key Bazel Targets
- `//:app` - Rust binary compiled to WASM
- `//:app_wasm` - WASM bindings generated via `wasm-bindgen`
- `//:app_wasm_opt` - Optimized WASM via `wasm-opt` (for production)
- `//:app_wasm_opt_br` - Brotli-compressed WASM
- `//:tailwind` - Generates TailwindCSS output
- `//server:server` - Development server binary
- `//server:opt` - Optimized server binary with wasm-opt'd assets
- `//server:image-amd64` - Docker image (linux/amd64)
- `//server:image-arm64` - Docker image (linux/arm64)
- `//bundle:bundle` - Vite bundle for JS assets

## Common Commands

### Development

```bash
# Run development server (without wasm-opt, faster builds)
bazel run //server:server

# Build everything
bazel build //:app //server:server

# Build with draft content visible
bazel build --//:show_drafts //:app
```

### Production Builds

```bash
# Build optimized server with wasm-opt (takes ~3s extra for wasm-opt)
bazel build //server:opt

# Run optimized server locally
bazel run //server:opt

# Build Docker images
bazel build //server:image-amd64
bazel build //server:image-arm64
```

### Frontend Development

```bash
# Rebuild TailwindCSS
bazel build //:tailwind

# Build WASM only
bazel build //:app_wasm

# Build optimized WASM
bazel build //:app_wasm_opt
```

### Testing

The project uses standard Rust testing. To run tests for a specific module:

```bash
# Test the main library
bazel test //:thockflow

# Test with Cargo (if needed outside Bazel)
cargo test
```

## Important Implementation Details

### WASM Hydration Flow
1. Server renders initial HTML via `yew::ServerRenderer` in `server/src/main.rs:56-69`
2. HTML includes WASM init script (generated in `server/src/main.rs:38-49`)
3. Browser loads WASM and JS, then `src/bin/app.rs` hydrates the pre-rendered DOM
4. This enables fast initial page load with SEO-friendly HTML

### Routing
- `Route` enum in `src/lib.rs:11-17` defines all routes
- `RoutableService` in `server/src/main.rs:121-198` routes known paths to SSR, unknown paths to static file serving
- Both client-side (`BrowserRouter`) and server-side (`MemoryHistory`) routers are configured

### WASM Optimization
- Development builds use unoptimized WASM for faster iteration
- Production builds (`//server:opt`) use `wasm-opt` and Brotli compression
- Environment variables control which assets are served:
  - `APP_WASM_PATH`: Path to WASM file (default: `/app_wasm_bg.wasm`, optimized: `/app_wasm_bg_opt.wasm`)
  - `AXUM_PRECOMPRESSED_WASM`: Enables Brotli pre-compressed WASM serving

### Workspace Structure
- Root package (`thockflow`): Main library + WASM binary
- `server` workspace member: Axum server that depends on root package
- Bazel manages both Cargo workspace and its own build graph

### TypeScript/Vite Bundle
- Lives in `bundle/` directory
- Compiled via Bazel using rules_js and Vite
- Output goes to `bundle/dist/` which server serves as static files
- Used for any non-Rust JavaScript dependencies

## Environment Variables

- `HTTP_LISTEN_ADDR`: Server listen address (default: `127.0.0.1:8080`)
- `SHOW_UNPUBLISHED`: Set to `1` to show draft content (controlled by `--//:show_drafts` flag)
- `APP_WASM_PATH`: Path to WASM file (set automatically in optimized builds)
- `AXUM_PRECOMPRESSED_WASM`: Enable Brotli pre-compressed WASM (set automatically in optimized builds)
