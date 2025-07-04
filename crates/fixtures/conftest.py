"""
Enhanced pytest configuration for LLM API fixture capture using pytest-recording.

This configuration provides:
- JSON format cassettes with rich metadata
- Timing information capture
- Streaming response preservation
- Hierarchical cassette organization
- Dual-mode testing (streaming and non-streaming)
"""

import os
import json
import time
import base64
from typing import Dict, Any, List, Optional
from datetime import datetime, timezone
import pytest
import vcr
from dotenv import load_dotenv
import serializer

# Load environment variables from .env file
load_dotenv()


# Store timing info globally to work with dict-like request objects
_request_timings = {}


def add_timing_metadata(request, response):
    """Add timing information to the response metadata."""
    # Handle both dict-like VCR requests and httpx Request objects
    if hasattr(request, 'method') and hasattr(request, 'url'):
        # httpx Request object
        method = request.method
        uri = str(request.url)
    else:
        # VCR dict format
        method = request.get('method', '')
        uri = request.get('uri', '')
    
    # Generate a unique request key
    request_key = f"{method}:{uri}"
    
    # Record timing in response headers (will be preserved in cassette)
    if request_key in _request_timings:
        elapsed_ms = int((time.time() - _request_timings[request_key]) * 1000)
        response['headers']['X-VCR-Elapsed-Ms'] = [str(elapsed_ms)]
        # Clean up
        del _request_timings[request_key]
    
    # Add timestamp
    response['headers']['X-VCR-Recorded-At'] = [datetime.now(timezone.utc).isoformat()]
    
    return response


def before_record_request(request):
    """Process request before recording."""
    # Handle both dict-like VCR requests and httpx Request objects
    if hasattr(request, 'method') and hasattr(request, 'url'):
        # httpx Request object
        method = request.method
        uri = str(request.url)
    else:
        # VCR dict format
        method = request.get('method', '')
        uri = request.get('uri', '')
    
    # Store timing marker
    request_key = f"{method}:{uri}"
    _request_timings[request_key] = time.time()
    
    # Only process headers for dict-like requests
    if isinstance(request, dict) and 'headers' in request:
        # Normalize header names to lowercase
        request['headers'] = {k.lower(): v for k, v in request['headers'].items()}
    
    return request


def before_record_response(response):
    """Process response before recording, preserving SSE structure."""
    # Add timestamp
    if 'headers' not in response:
        response['headers'] = {}
    response['headers']['X-VCR-Recorded-At'] = [datetime.now(timezone.utc).isoformat()]
    
    # For SSE responses, ensure we capture the raw event stream
    content_type = ''
    if 'headers' in response:
        for header, value in response['headers'].items():
            if header.lower() == 'content-type':
                content_type = value[0] if isinstance(value, list) else value
                break
    
    # Mark SSE responses for special handling
    if 'text/event-stream' in content_type:
        response['headers']['X-VCR-SSE-Response'] = ['true']
    
    return response


@pytest.fixture(scope='module')
def vcr_config():
    """Configure VCR for all tests using pytest-recording."""
    return {
        'serializer': 'json',
        'record_mode': os.environ.get('VCR_RECORD_MODE', 'once'),
        'match_on': ['method', 'scheme', 'host', 'port', 'path', 'query'],
        'filter_headers': [
            ('authorization', 'REDACTED'),
            ('x-api-key', 'REDACTED'),
            ('api-key', 'REDACTED'),
            ('openai-api-key', 'REDACTED'),
        ],
        'decode_compressed_response': True,
        'before_record_request': before_record_request,
        'before_record_response': before_record_response,
    }


@pytest.fixture
def vcr_cassette_dir(request):
    """Organize cassettes by provider and API."""
    # Get test file path
    test_file = request.node.fspath
    test_name = test_file.purebasename
    
    # Determine provider and API from test file name
    if 'openai' in test_name:
        if 'chat_completions' in test_name:
            subdir = 'openai/chat_completions'
        elif 'responses' in test_name:
            subdir = 'openai/responses'
        else:
            subdir = 'openai/other'
    elif 'anthropic' in test_name:
        subdir = 'anthropic/messages'
    else:
        subdir = 'other'
    
    cassette_dir = os.path.join('data', subdir)
    os.makedirs(cassette_dir, exist_ok=True)
    return cassette_dir


def pytest_recording_configure(config, vcr):
    """Hook to configure VCR instance with our custom serializer."""
    # Register the custom serializer from serializer.py
    vcr.register_serializer('json', serializer)


@pytest.fixture
def default_cassette_name(request):
    """Generate cassette name with streaming suffix when applicable."""
    test_name = request.node.name
    
    # Remove test_ prefix if present
    if test_name.startswith('test_'):
        test_name = test_name[5:]
    
    # Check if this is a streaming test
    if hasattr(request, 'param') and getattr(request.param, 'streaming', False):
        test_name += '_streaming'
    
    # Handle parametrized tests
    if '[' in test_name:
        # Clean up parameter names for filesystem
        test_name = test_name.replace('[', '_').replace(']', '').replace('-', '_')
    
    return test_name + '.json'  # Use .json extension


class APITestConfig:
    """Configuration for API tests."""
    
    def __init__(self, streaming: bool = False):
        self.streaming = streaming
        self.api_key_env_var = None
        self.skip_if_no_key = True


@pytest.fixture(params=[
    APITestConfig(streaming=False),
    APITestConfig(streaming=True)
], ids=['non_streaming', 'streaming'])
def api_mode(request):
    """Fixture that provides both streaming and non-streaming test modes."""
    return request.param


@pytest.fixture
def check_api_key(vcr_config):
    """Helper fixture to check for API keys."""
    def _check(env_var: str, skip: bool = True):
        api_key = os.environ.get(env_var)
        # In replay mode (once), we can use a dummy key since VCR will replay
        if not api_key:
            record_mode = vcr_config.get('record_mode', 'once')
            if record_mode == 'once':
                # Use dummy key for replay mode
                return "dummy-key-for-vcr-replay"
            elif skip:
                pytest.skip(f"{env_var} environment variable not set")
        return api_key
    return _check


# Environment variable validation
@pytest.fixture(scope='session', autouse=True)
def validate_environment():
    """Validate test environment and provide warnings."""
    warnings = []
    
    # Check for API keys
    if not os.environ.get('OPENAI_API_KEY'):
        warnings.append("OPENAI_API_KEY not set - OpenAI tests will be skipped")
    
    if not os.environ.get('ANTHROPIC_API_KEY'):
        warnings.append("ANTHROPIC_API_KEY not set - Anthropic tests will be skipped")
    
    # Check for cassette directory
    if not os.path.exists('data'):
        os.makedirs('data')
    
    # Print warnings
    if warnings:
        print("\n=== Test Environment Warnings ===")
        for warning in warnings:
            print(f"  ⚠️  {warning}")
        print("=================================\n")


@pytest.fixture
def sample_image_base64():
    """Provide a small test image in base64 format."""
    # 1x1 red pixel PNG
    return "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8/5+hHgAHggJ/PchI7wAAAABJRU5ErkJggg=="