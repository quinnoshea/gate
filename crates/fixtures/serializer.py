"""
Enhanced JSON serializer for VCR with metadata and binary support.
"""

import json
import base64
from datetime import datetime, timezone
import vcr


def serialize(cassette_dict):
    """Serialize cassette data to JSON with enhanced metadata."""
    # Add cassette-level metadata
    cassette_dict['metadata'] = {
        'vcr_version': vcr.__version__,
        'recorded_at': datetime.now(timezone.utc).isoformat(),
    }
    
    # Process interactions to handle binary data
    for interaction in cassette_dict.get('interactions', []):
        request = interaction.get('request', {})
        response = interaction.get('response', {})
        
        # Handle request body
        if 'body' in request and request['body'] is not None:
            if isinstance(request['body'], bytes):
                request['body'] = {
                    '_base64': base64.b64encode(request['body']).decode('utf-8')
                }
            elif isinstance(request['body'], str):
                # Keep string as is
                pass
        
        # Handle response body
        if 'body' in response:
            body = response['body']
            if isinstance(body, dict) and 'string' in body:
                # This is VCR's format for response bodies
                if isinstance(body['string'], bytes):
                    body['string'] = base64.b64encode(body['string']).decode('utf-8')
                    body['encoding'] = 'base64'
            elif isinstance(body, bytes):
                response['body'] = {
                    'string': base64.b64encode(body).decode('utf-8'),
                    'encoding': 'base64'
                }
    
    return json.dumps(cassette_dict, indent=2, sort_keys=True)


def deserialize(cassette_string):
    """Deserialize JSON cassette data."""
    cassette_dict = json.loads(cassette_string)
    
    # Process interactions to decode binary data
    for interaction in cassette_dict.get('interactions', []):
        request = interaction.get('request', {})
        response = interaction.get('response', {})
        
        # Handle request body
        if isinstance(request.get('body'), dict) and '_base64' in request['body']:
            request['body'] = base64.b64decode(request['body']['_base64'])
        
        # Handle response body
        if 'body' in response:
            body = response['body']
            if isinstance(body, dict):
                if body.get('encoding') == 'base64' and 'string' in body:
                    body['string'] = base64.b64decode(body['string'])
                    del body['encoding']
    
    return cassette_dict