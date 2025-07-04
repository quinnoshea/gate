# gate-frontend

Web frontend for Gate built with Yew (Rust/WASM). Provides a browser-based interface for managing API keys, viewing usage, and testing inference.

## Responsibilities

- **User Dashboard**: API key management, usage metrics
- **WebAuthn Integration**: Hardware authentication in browser
- **Chat Interface**: Test inference endpoints via `gate-chat-ui`
- **Model Explorer**: Browse available models and providers
- **WASM Client**: Uses `gate-http` client in browser

## Build

```bash
make frontend-dev   # Development server on :8081
make frontend-build # Production build to dist/
```

## Organization

```
src/
├── components/      # Reusable UI components
├── pages/          # Main application pages
├── services/       # API client and state management
└── utils/          # WebAuthn and other utilities

assets/            # Static files (CSS, images)
index.html         # Entry point
Trunk.toml         # Build configuration
```

## Tech Stack

- **Yew**: Rust framework for WASM web apps
- **Tailwind CSS**: Utility-first styling
- **Trunk**: WASM bundler and dev server
- **web-sys**: Browser API bindings

## Features

- Single Page Application (SPA)
- Client-side routing
- Local storage for settings
- WebAuthn for passwordless login
- Real-time usage updates

## Dependencies

- `yew`: Frontend framework
- `gate-chat-ui`: Reusable chat component
- `gate-http`: API client (client feature only)
- `web-sys`: Browser APIs (WebAuthn, storage, etc.)
- `gloo`: Yew utilities

## Development

Uses Trunk for hot-reload development:
```toml
# Trunk.toml
[[proxy]]
backend = "http://localhost:31145"
```

Proxies API calls to local Gate daemon.

## Production Build

Built files in `dist/`:
- `index.html`: Entry point
- `gate-frontend-*.wasm`: Application code
- `gate-frontend-*.js`: WASM loader

Serve with any static file server or embed in `gate-daemon`.

## Risks

- **Browser Compatibility**: WebAuthn requires modern browsers
- **WASM Size**: Bundle size impacts load time
- **API Changes**: Must stay synchronized with backend