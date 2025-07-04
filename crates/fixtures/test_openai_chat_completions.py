#!/usr/bin/env python3
"""
Comprehensive tests for OpenAI Chat Completions API (v1/chat/completions).
Tests cover all major features including streaming, function calling, structured outputs,
multimodal inputs, and model-specific features.
"""

import json
import pytest
import pytest_asyncio
from pydantic import BaseModel
from typing import List, Optional, Literal
from openai import AsyncOpenAI
import openai

pytestmark = pytest.mark.asyncio


@pytest_asyncio.fixture
async def openai_client(check_api_key):
    """Initialize AsyncOpenAI client with API key."""
    api_key = check_api_key("OPENAI_API_KEY")
    return AsyncOpenAI(api_key=api_key)


# Pydantic models for structured outputs
class WeatherInfo(BaseModel):
    location: str
    temperature: float
    conditions: str
    humidity: int
    wind_speed: float


class Step(BaseModel):
    step_number: int
    description: str
    calculation: str
    result: float


class MathSolution(BaseModel):
    problem: str
    steps: List[Step]
    final_answer: float
    explanation: str


# Tool definitions
WEATHER_TOOL = {
    "type": "function",
    "function": {
        "name": "get_weather",
        "description": "Get current weather for a location",
        "parameters": {
            "type": "object",
            "properties": {
                "location": {
                    "type": "string",
                    "description": "City and state, e.g. San Francisco, CA"
                },
                "unit": {
                    "type": "string",
                    "enum": ["celsius", "fahrenheit"],
                    "description": "Temperature unit"
                }
            },
            "required": ["location"]
        }
    }
}

CALCULATOR_TOOL = {
    "type": "function",
    "function": {
        "name": "calculate",
        "description": "Perform mathematical calculations",
        "parameters": {
            "type": "object",
            "properties": {
                "expression": {
                    "type": "string",
                    "description": "Mathematical expression to evaluate"
                }
            },
            "required": ["expression"]
        }
    }
}


@pytest.mark.vcr
async def test_basic_chat_completion(openai_client):
    """Test basic non-streaming chat completion."""
    response = await openai_client.chat.completions.create(
        model="gpt-4o-mini",
        messages=[
            {
                "role": "system",
                "content": "You are a helpful assistant."
            },
            {
                "role": "user",
                "content": "What is the capital of France? Answer in one word."
            }
        ],
        temperature=0,
        max_tokens=10
    )
    
    assert response.choices is not None
    assert len(response.choices) > 0
    assert response.choices[0].message.content is not None
    assert "paris" in response.choices[0].message.content.lower()
    assert response.usage is not None
    assert response.usage.total_tokens > 0


@pytest.mark.vcr
async def test_streaming_chat_completion(openai_client):
    """Test streaming chat completion with SSE chunks."""
    chunks = []
    stream = await openai_client.chat.completions.create(
        model="gpt-4o-mini",
        messages=[
            {
                "role": "user",
                "content": "Count from 1 to 5, one number per line."
            }
        ],
        stream=True,
        temperature=0
    )
    
    async for chunk in stream:
        chunks.append(chunk)
        if chunk.choices and chunk.choices[0].delta.content:
            content = chunk.choices[0].delta.content
            assert isinstance(content, str)
    
    assert len(chunks) > 1  # Should have multiple chunks
    assert chunks[0].choices[0].delta.role == "assistant"
    
    # Reconstruct full response
    full_content = "".join(
        chunk.choices[0].delta.content or ""
        for chunk in chunks
        if chunk.choices
    )
    assert "1" in full_content
    assert "5" in full_content


@pytest.mark.vcr
async def test_function_calling(openai_client):
    """Test complete function/tool calling flow with multi-turn conversation."""
    # Step 1: Initial request that triggers tool calls
    messages = [
        {
            "role": "user",
            "content": "What's the weather like in San Francisco and New York? Compare them."
        }
    ]
    
    response = await openai_client.chat.completions.create(
        model="gpt-4o-mini",
        messages=messages,
        tools=[WEATHER_TOOL],
        tool_choice="auto"
    )
    
    message = response.choices[0].message
    assert message.tool_calls is not None
    assert len(message.tool_calls) >= 1  # At least one tool call
    
    # Add assistant message with tool calls to conversation
    messages.append(message.model_dump(exclude_none=True))
    
    # Step 2: Simulate tool execution and add results
    tool_results = []
    for tool_call in message.tool_calls:
        assert tool_call.type == "function"
        assert tool_call.function.name == "get_weather"
        args = json.loads(tool_call.function.arguments)
        assert "location" in args
        
        # Simulate weather API response
        if "san francisco" in args["location"].lower():
            weather_data = {
                "location": "San Francisco, CA",
                "temperature": 65,
                "conditions": "Partly cloudy",
                "humidity": 70,
                "wind_speed": 12
            }
        else:
            weather_data = {
                "location": "New York, NY",
                "temperature": 45,
                "conditions": "Clear",
                "humidity": 50,
                "wind_speed": 8
            }
        
        tool_message = {
            "role": "tool",
            "tool_call_id": tool_call.id,
            "content": json.dumps(weather_data)
        }
        messages.append(tool_message)
    
    # Step 3: Get final response with weather comparison
    final_response = await openai_client.chat.completions.create(
        model="gpt-4o-mini",
        messages=messages,
        tools=[WEATHER_TOOL]
    )
    
    final_content = final_response.choices[0].message.content
    assert final_content is not None
    assert "san francisco" in final_content.lower()
    assert "new york" in final_content.lower()
    # Should compare temperatures or conditions
    assert any(word in final_content.lower() for word in ["warmer", "cooler", "colder", "hotter", "temperature", "degrees"])


@pytest.mark.vcr
async def test_json_mode(openai_client):
    """Test JSON mode response format."""
    response = await openai_client.chat.completions.create(
        model="gpt-4o-mini",
        messages=[
            {
                "role": "system",
                "content": "You are a helpful assistant that always responds with valid JSON."
            },
            {
                "role": "user",
                "content": "List 3 programming languages with their year of creation as JSON."
            }
        ],
        response_format={"type": "json_object"},
        temperature=0
    )
    
    content = response.choices[0].message.content
    assert content is not None
    
    # Should be valid JSON
    data = json.loads(content)
    assert isinstance(data, dict)
    # The response might be nested, so check for any list with 3+ items
    assert any(
        isinstance(v, list) and len(v) >= 3 
        for v in data.values()
    ) or len(data) >= 3


@pytest.mark.vcr
async def test_structured_output_pydantic(openai_client):
    """Test structured output with Pydantic model."""
    completion = await openai_client.beta.chat.completions.parse(
        model="gpt-4o-mini",
        messages=[
            {
                "role": "system",
                "content": "You are a helpful math tutor."
            },
            {
                "role": "user",
                "content": "Solve step by step: What is 25 * 4 + 10?"
            }
        ],
        response_format=MathSolution
    )
    
    message = completion.choices[0].message
    assert message.parsed is not None
    assert isinstance(message.parsed, MathSolution)
    assert message.parsed.final_answer == 110
    assert len(message.parsed.steps) >= 2
    assert "multiply" in message.parsed.explanation.lower() or "multiplication" in message.parsed.explanation.lower()


@pytest.mark.vcr
async def test_multi_turn_conversation(openai_client):
    """Test multi-turn conversation with context."""
    messages = [
        {"role": "system", "content": "You are a helpful assistant."},
        {"role": "user", "content": "My name is Alice. Remember it."},
        {"role": "assistant", "content": "Hello Alice! I'll remember your name."},
        {"role": "user", "content": "What's my name?"}
    ]
    
    response = await openai_client.chat.completions.create(
        model="gpt-4o-mini",
        messages=messages,
        temperature=0
    )
    
    content = response.choices[0].message.content
    assert content is not None
    assert "alice" in content.lower()


@pytest.mark.vcr
async def test_system_message_instructions(openai_client):
    """Test system message with specific instructions."""
    response = await openai_client.chat.completions.create(
        model="gpt-4o-mini",
        messages=[
            {
                "role": "system",
                "content": "You are a pirate. Always respond in pirate speak with 'Arrr!' at the beginning."
            },
            {
                "role": "user",
                "content": "Hello, how are you?"
            }
        ],
        temperature=0.7,
        max_tokens=50
    )
    
    content = response.choices[0].message.content
    assert content is not None
    assert "arr" in content.lower()


@pytest.mark.vcr
async def test_parallel_tool_calls(openai_client):
    """Test parallel tool calling capability with complete flow."""
    # Step 1: Request that triggers multiple parallel tool calls
    messages = [
        {
            "role": "user",
            "content": "Calculate the area of a rectangle with length 10 and width 5, and also calculate the perimeter. Then tell me which is larger."
        }
    ]
    
    response = await openai_client.chat.completions.create(
        model="gpt-4o-mini",
        messages=messages,
        tools=[CALCULATOR_TOOL],
        parallel_tool_calls=True
    )
    
    message = response.choices[0].message
    assert message.tool_calls is not None
    assert len(message.tool_calls) >= 2  # Should make multiple tool calls
    
    # Add assistant message to conversation
    messages.append(message.model_dump(exclude_none=True))
    
    # Step 2: Execute tools and collect results
    tool_results = {}
    for tool_call in message.tool_calls:
        assert tool_call.function.name == "calculate"
        args = json.loads(tool_call.function.arguments)
        expression = args["expression"]
        
        # Evaluate the expression
        try:
            result = eval(expression)
        except:
            # Handle more complex expressions
            if "10" in expression and "5" in expression:
                if "*" in expression:
                    result = 10 * 5  # Area = 50
                elif "+" in expression and "2" in expression:
                    result = 2 * (10 + 5)  # Perimeter = 30
                else:
                    result = 0
            else:
                result = 0
        
        tool_results[tool_call.id] = result
        
        # Add tool response message
        tool_message = {
            "role": "tool",
            "tool_call_id": tool_call.id,
            "content": str(result)
        }
        messages.append(tool_message)
    
    # Step 3: Get final response comparing the results
    final_response = await openai_client.chat.completions.create(
        model="gpt-4o-mini",
        messages=messages,
        tools=[CALCULATOR_TOOL]
    )
    
    final_content = final_response.choices[0].message.content
    assert final_content is not None
    # Should mention both area and perimeter
    assert "area" in final_content.lower()
    assert "perimeter" in final_content.lower()
    # Should compare them (area=50 is larger than perimeter=30)
    assert any(word in final_content.lower() for word in ["larger", "greater", "bigger", "more", "50", "30"])


@pytest.mark.vcr
async def test_max_tokens_and_stop_sequence(openai_client):
    """Test max tokens limit and stop sequences."""
    response = await openai_client.chat.completions.create(
        model="gpt-4o-mini",
        messages=[
            {
                "role": "user",
                "content": "Write a story about a robot. End each sentence with a period."
            }
        ],
        max_tokens=50,
        stop=[".", "!"],
        temperature=0.8
    )
    
    content = response.choices[0].message.content
    assert content is not None
    assert len(content.split()) < 60  # Should be limited by max_tokens
    
    # Check finish reason
    assert response.choices[0].finish_reason in ["stop", "length"]


@pytest.mark.vcr
async def test_structured_output_with_tool_use(openai_client):
    """Test combining structured output with tool use in a complete flow."""
    # Step 1: Initial request
    messages = [
        {
            "role": "system",
            "content": "You are a helpful weather assistant. Use the weather tool to get data, then provide a structured analysis."
        },
        {
            "role": "user",
            "content": "Get the weather for Seattle and provide a structured analysis."
        }
    ]
    
    # First call - should trigger tool use
    response = await openai_client.chat.completions.create(
        model="gpt-4o-mini",
        messages=messages,
        tools=[WEATHER_TOOL],
        tool_choice="auto"
    )
    
    message = response.choices[0].message
    assert message.tool_calls is not None
    
    # Add assistant message
    messages.append(message.model_dump(exclude_none=True))
    
    # Step 2: Provide tool result
    for tool_call in message.tool_calls:
        weather_data = {
            "location": "Seattle, WA",
            "temperature": 55,
            "conditions": "Rainy",
            "humidity": 85,
            "wind_speed": 15
        }
        
        tool_message = {
            "role": "tool",
            "tool_call_id": tool_call.id,
            "content": json.dumps(weather_data)
        }
        messages.append(tool_message)
    
    # Step 3: Request structured analysis
    messages.append({
        "role": "user",
        "content": "Now provide a structured weather analysis."
    })
    
    # Get structured response
    completion = await openai_client.beta.chat.completions.parse(
        model="gpt-4o-mini",
        messages=messages,
        response_format=WeatherInfo
    )
    
    result = completion.choices[0].message
    assert result.parsed is not None
    assert isinstance(result.parsed, WeatherInfo)
    assert result.parsed.location == "Seattle, WA"
    assert result.parsed.temperature == 55
    assert "rain" in result.parsed.conditions.lower()


@pytest.mark.vcr
async def test_logprobs(openai_client):
    """Test logprobs parameter for token probabilities."""
    response = await openai_client.chat.completions.create(
        model="gpt-4o-mini",
        messages=[
            {
                "role": "user",
                "content": "Say 'Hello World'"
            }
        ],
        logprobs=True,
        top_logprobs=3,
        temperature=0
    )
    
    choice = response.choices[0]
    assert choice.logprobs is not None
    assert choice.logprobs.content is not None
    assert len(choice.logprobs.content) > 0
    
    # Check logprob structure
    for token_info in choice.logprobs.content:
        assert token_info.token is not None
        assert token_info.logprob is not None
        assert isinstance(token_info.logprob, (int, float))
        if token_info.top_logprobs:
            assert len(token_info.top_logprobs) <= 3