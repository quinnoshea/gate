# Gate Chat UI Demo

This demo showcases the Gate Chat UI component with support for loading real API conversation cassettes.

## Running the Demo

```bash
trunk serve
```

This will start the demo app on http://localhost:8081.

## Features

- **Cassette Loading**: Load real API conversations from test cassettes
- **Dark Mode**: Toggle between light and dark themes  
- **Multimodal Support**: Send messages with images and file attachments
- **Streaming Display**: View streaming responses from cassettes

## Available Cassettes

The demo includes example cassettes from:
- OpenAI Chat Completions API (basic, streaming, multi-turn)
- Anthropic Messages API (basic, streaming, vision)

Select a cassette from the dropdown in the header to load and view the conversation.

## How it Works

The cassettes are symlinked from the fixtures directory and served as static files by trunk. This keeps the WASM bundle small while still providing access to all test data.