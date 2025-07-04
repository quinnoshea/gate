#!/usr/bin/env python3
"""
Comprehensive tests for Anthropic Messages API.
Tests cover all major features including streaming, tool use, vision,
multi-turn conversations, and advanced features.
"""

import json
import pytest
import pytest_asyncio
from typing import List, Optional, Literal, Union
from anthropic import AsyncAnthropic
from anthropic.types import MessageParam, ToolParam

pytestmark = pytest.mark.asyncio


@pytest_asyncio.fixture
async def anthropic_client(check_api_key):
    """Initialize AsyncAnthropic client with API key."""
    api_key = check_api_key("ANTHROPIC_API_KEY")
    return AsyncAnthropic(api_key=api_key)


# Tool definitions
WEATHER_TOOL: ToolParam = {
    "name": "get_weather",
    "description": "Get current weather for a location",
    "input_schema": {
        "type": "object",
        "properties": {
            "location": {
                "type": "string",
                "description": "City and state, e.g. San Francisco, CA"
            },
            "unit": {
                "type": "string",
                "enum": ["celsius", "fahrenheit"],
                "description": "Temperature unit",
                "default": "fahrenheit"
            }
        },
        "required": ["location"]
    }
}

CALCULATOR_TOOL: ToolParam = {
    "name": "calculate",
    "description": "Perform mathematical calculations",
    "input_schema": {
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


@pytest.mark.vcr
async def test_basic_message(anthropic_client):
    """Test basic message request and response."""
    response = await anthropic_client.messages.create(
        model="claude-sonnet-4-0",
        max_tokens=100,
        messages=[
            {
                "role": "user",
                "content": "What is the capital of France? Answer in one word."
            }
        ],
        temperature=0
    )
    
    assert response.role == "assistant"
    assert response.content is not None
    assert len(response.content) > 0
    assert response.content[0].type == "text"
    assert "paris" in response.content[0].text.lower()
    assert response.usage.input_tokens > 0
    assert response.usage.output_tokens > 0


@pytest.mark.vcr
async def test_tool_use_multi_turn(anthropic_client):
    """Test complete tool use flow with multi-turn conversation."""
    # Step 1: Initial request that triggers tool use
    messages: List[MessageParam] = [
        {
            "role": "user",
            "content": "What's the weather like in San Francisco and New York? Compare them."
        }
    ]
    
    response = await anthropic_client.messages.create(
        model="claude-sonnet-4-0",
        max_tokens=1024,
        messages=messages,
        tools=[WEATHER_TOOL]
    )
    
    assert response.stop_reason == "tool_use"
    assert any(block.type == "tool_use" for block in response.content)
    
    # Add assistant response to conversation
    messages.append({
        "role": "assistant",
        "content": response.content
    })
    
    # Step 2: Process tool calls and add results
    tool_results = []
    for block in response.content:
        if block.type == "tool_use":
            # Simulate weather API response
            if "san francisco" in block.input.get("location", "").lower():
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
            
            tool_results.append({
                "type": "tool_result",
                "tool_use_id": block.id,
                "content": json.dumps(weather_data)
            })
    
    # Add tool results as user message
    messages.append({
        "role": "user",
        "content": tool_results
    })
    
    # Step 3: Get final response with weather comparison
    final_response = await anthropic_client.messages.create(
        model="claude-sonnet-4-0",
        max_tokens=1024,
        messages=messages,
        tools=[WEATHER_TOOL]
    )
    
    assert final_response.role == "assistant"
    assert final_response.content[0].type == "text"
    final_text = final_response.content[0].text.lower()
    assert "san francisco" in final_text
    assert "new york" in final_text
    # Should compare temperatures or weather
    assert any(word in final_text for word in ["warmer", "cooler", "temperature", "degrees"])


@pytest.mark.vcr
async def test_streaming_basic(anthropic_client):
    """Test streaming with multi-turn conversation."""
    messages: List[MessageParam] = [
        {
            "role": "user", 
            "content": "Count from 1 to 5, one number per line."
        }
    ]
    
    # First streaming response
    chunks = []
    text_chunks = []
    
    async with anthropic_client.messages.stream(
        model="claude-sonnet-4-0",
        max_tokens=100,
        messages=messages,
        temperature=0
    ) as stream:
        async for event in stream:
            chunks.append(event)
            if event.type == "content_block_delta" and event.delta.type == "text_delta":
                text_chunks.append(event.delta.text)
    
    # Verify streaming worked
    assert len(chunks) > 1
    event_types = {event.type for event in chunks}
    assert "message_start" in event_types
    assert "content_block_delta" in event_types
    
    full_text = "".join(text_chunks)
    assert "1" in full_text
    assert "5" in full_text
    
    # Get accumulated message for conversation
    first_response = await stream.get_final_message()
    messages.append({"role": first_response.role, "content": first_response.content})
    
    # Follow-up question
    messages.append({
        "role": "user",
        "content": "What number comes after 5?"
    })
    
    # Second streaming response
    async with anthropic_client.messages.stream(
        model="claude-sonnet-4-0",
        max_tokens=50,
        messages=messages
    ) as stream2:
        follow_up_text = ""
        async for event in stream2:
            if event.type == "content_block_delta" and event.delta.type == "text_delta":
                follow_up_text += event.delta.text
    
    # Should mention 6
    assert "6" in follow_up_text or "six" in follow_up_text.lower()


@pytest.mark.vcr
async def test_multi_turn_conversation(anthropic_client):
    """Test multi-turn conversation with context retention."""
    messages: List[MessageParam] = [
        {
            "role": "user",
            "content": "My name is Alice and I love astronomy. Remember this."
        }
    ]
    
    # First response
    response1 = await anthropic_client.messages.create(
        model="claude-sonnet-4-0",
        max_tokens=150,
        messages=messages
    )
    
    # Add to conversation
    messages.append({"role": "assistant", "content": response1.content})
    messages.append({
        "role": "user",
        "content": "What's my name and what do I love?"
    })
    
    # Second response should remember context
    response2 = await anthropic_client.messages.create(
        model="claude-sonnet-4-0",
        max_tokens=150,
        messages=messages
    )
    
    response_text = response2.content[0].text.lower()
    assert "alice" in response_text
    assert "astronomy" in response_text


@pytest.mark.vcr
async def test_vision_image_analysis(anthropic_client):
    """Test vision capabilities with multi-turn conversation."""
    # Use a real cat image URL from placecats.com
    messages: List[MessageParam] = [
        {
            "role": "user",
            "content": [
                {
                    "type": "text",
                    "text": "What animal is in this image? Answer in one word."
                },
                {
                    "type": "image",
                    "source": {
                        "type": "url",
                        "url": "https://placecats.com/200/200"
                    }
                }
            ]
        }
    ]
    
    # First response about the animal
    response = await anthropic_client.messages.create(
        model="claude-sonnet-4-0",
        max_tokens=200,
        messages=messages
    )
    
    assert response.role == "assistant"
    assert response.content[0].type == "text"
    assert "cat" in response.content[0].text.lower()
    
    # Add to conversation
    messages.append({"role": response.role, "content": response.content})
    
    # Follow-up about the image
    messages.append({
        "role": "user",
        "content": "What is the cat doing? Is it sleeping, sitting, or playing?"
    })
    
    # Second response should provide more details about the cat
    response2 = await anthropic_client.messages.create(
        model="claude-sonnet-4-0",
        max_tokens=200,
        messages=messages
    )
    
    response2_text = response2.content[0].text.lower()
    # Should describe the cat's activity
    assert any(word in response2_text for word in ["sitting", "sleeping", "playing", "lying", "standing", "looking"])


@pytest.mark.vcr
async def test_system_prompt(anthropic_client):
    """Test system prompt with multi-turn conversation."""
    system_prompt = "You are a pirate. Always respond in pirate speak, starting with 'Arrr!'"
    
    messages: List[MessageParam] = [
        {
            "role": "user",
            "content": "Hello, how are you today?"
        }
    ]
    
    # First response with pirate system prompt
    response = await anthropic_client.messages.create(
        model="claude-sonnet-4-0",
        max_tokens=100,
        system=system_prompt,
        messages=messages,
        temperature=0.7
    )
    
    text = response.content[0].text.lower()
    assert "arr" in text or "ahoy" in text
    
    # Add to conversation
    messages.append({"role": response.role, "content": response.content})
    
    # Follow-up to verify pirate persona is maintained
    messages.append({
        "role": "user",
        "content": "What's your favorite treasure?"
    })
    
    # Second response should still be pirate-themed
    response2 = await anthropic_client.messages.create(
        model="claude-sonnet-4-0",
        max_tokens=150,
        system=system_prompt,
        messages=messages,
        temperature=0.7
    )
    
    text2 = response2.content[0].text.lower()
    # Should maintain pirate speak and mention treasure-related terms
    assert any(word in text2 for word in ["arr", "gold", "doubloon", "treasure", "booty", "plunder"])


@pytest.mark.vcr
async def test_parallel_tool_calls(anthropic_client):
    """Test multiple tool calls in a single response."""
    messages: List[MessageParam] = [
        {
            "role": "user",
            "content": "Calculate the area of a rectangle with length 10 and width 5, and also calculate the perimeter. Show me both calculations."
        }
    ]
    
    # First response with tool calls
    response = await anthropic_client.messages.create(
        model="claude-sonnet-4-0",
        max_tokens=1024,
        messages=messages,
        tools=[CALCULATOR_TOOL]
    )
    
    # Count tool use blocks
    tool_uses = [block for block in response.content if block.type == "tool_use"]
    assert len(tool_uses) >= 2  # Should call calculator at least twice
    
    # Add assistant response
    messages.append({"role": "assistant", "content": response.content})
    
    # Process tool calls
    tool_results = []
    for block in response.content:
        if block.type == "tool_use":
            expression = block.input.get("expression", "")
            try:
                result = eval(expression)
            except:
                # Handle specific calculations
                if "10" in expression and "5" in expression:
                    if "*" in expression:
                        result = 50  # Area
                    elif "+" in expression:
                        result = 30  # Perimeter sum
                    else:
                        result = 0
                else:
                    result = 0
            
            tool_results.append({
                "type": "tool_result",
                "tool_use_id": block.id,
                "content": str(result)
            })
    
    # Add results
    messages.append({"role": "user", "content": tool_results})
    
    # Final response
    final_response = await anthropic_client.messages.create(
        model="claude-sonnet-4-0",
        max_tokens=1024,
        messages=messages,
        tools=[CALCULATOR_TOOL]
    )
    
    final_text = final_response.content[0].text.lower()
    assert "area" in final_text
    assert "perimeter" in final_text
    # Should mention the actual values
    assert any(str(num) in final_text for num in ["50", "30"])


@pytest.mark.vcr 
async def test_stop_sequences(anthropic_client):
    """Test stop sequences functionality."""
    response = await anthropic_client.messages.create(
        model="claude-sonnet-4-0",
        max_tokens=200,
        messages=[
            {
                "role": "user",
                "content": "Write a short story. End each sentence with a period."
            }
        ],
        stop_sequences=[".", "!", "?"],
        temperature=0.8
    )
    
    text = response.content[0].text
    # Should stop at first sentence ending
    assert text.count(".") <= 1
    assert text.count("!") <= 1
    assert text.count("?") <= 1
    
    # Check stop reason
    assert response.stop_reason in ["stop_sequence", "max_tokens", "end_turn"]


@pytest.mark.vcr
async def test_streaming_with_tool_use(anthropic_client):
    """Test complete streaming flow with tool use."""
    messages: List[MessageParam] = [
        {
            "role": "user",
            "content": "What's the weather in Paris? Then calculate 25 * 4."
        }
    ]
    
    # First streaming response with tool calls
    tool_use_blocks = []
    
    async with anthropic_client.messages.stream(
        model="claude-sonnet-4-0",
        max_tokens=1024,
        messages=messages,
        tools=[WEATHER_TOOL, CALCULATOR_TOOL]
    ) as stream:
        async for event in stream:
            if event.type == "content_block_start" and hasattr(event, "content_block"):
                if event.content_block.type == "tool_use":
                    tool_use_blocks.append(event.content_block)
    
    # Get accumulated message
    first_response = await stream.get_final_message()
    assert first_response.stop_reason == "tool_use"
    assert len(tool_use_blocks) >= 1
    
    # Add assistant response to conversation
    messages.append({"role": first_response.role, "content": first_response.content})
    
    # Process tool calls and add results
    tool_results = []
    for block in first_response.content:
        if block.type == "tool_use":
            if block.name == "get_weather":
                result = json.dumps({
                    "location": "Paris, France",
                    "temperature": 18,
                    "conditions": "Sunny",
                    "humidity": 60,
                    "wind_speed": 10
                })
            elif block.name == "calculate":
                result = "100"  # 25 * 4
            else:
                result = "Unknown tool"
            
            tool_results.append({
                "type": "tool_result",
                "tool_use_id": block.id,
                "content": result
            })
    
    # Add tool results
    messages.append({"role": "user", "content": tool_results})
    
    # Stream final response
    final_text = ""
    async with anthropic_client.messages.stream(
        model="claude-sonnet-4-0",
        max_tokens=1024,
        messages=messages,
        tools=[WEATHER_TOOL, CALCULATOR_TOOL]
    ) as stream2:
        async for event in stream2:
            if event.type == "content_block_delta" and event.delta.type == "text_delta":
                final_text += event.delta.text
    
    # Should mention both Paris weather and calculation result
    final_text_lower = final_text.lower()
    assert "paris" in final_text_lower
    assert "100" in final_text or "hundred" in final_text_lower


@pytest.mark.vcr
async def test_tool_choice_options(anthropic_client):
    """Test different tool choice options with complete flows."""
    # Test 1: tool_choice="auto" with multi-turn
    messages_auto: List[MessageParam] = [
        {
            "role": "user",
            "content": "Tell me a joke about programming."
        }
    ]
    
    response_auto = await anthropic_client.messages.create(
        model="claude-sonnet-4-0",
        max_tokens=200,
        messages=messages_auto,
        tools=[WEATHER_TOOL],
        tool_choice={"type": "auto"}
    )
    
    # Should not use tool for a joke request
    assert response_auto.stop_reason == "end_turn"
    assert all(block.type != "tool_use" for block in response_auto.content)
    
    # Add to conversation and ask something that needs the tool
    messages_auto.append({"role": response_auto.role, "content": response_auto.content})
    messages_auto.append({
        "role": "user",
        "content": "Thanks! Now what's the weather in London?"
    })
    
    response_auto2 = await anthropic_client.messages.create(
        model="claude-sonnet-4-0",
        max_tokens=200,
        messages=messages_auto,
        tools=[WEATHER_TOOL],
        tool_choice={"type": "auto"}
    )
    
    # Now should use the tool
    assert response_auto2.stop_reason == "tool_use"
    assert any(block.type == "tool_use" for block in response_auto2.content)
    
    # Test 2: tool_choice="any" forces tool use with complete flow
    messages_any: List[MessageParam] = [
        {
            "role": "user",
            "content": "What is 15 plus 27?"
        }
    ]
    
    response_any = await anthropic_client.messages.create(
        model="claude-sonnet-4-0",
        max_tokens=200,
        messages=messages_any,
        tools=[CALCULATOR_TOOL],
        tool_choice={"type": "any"}
    )
    
    # Should use tool
    assert response_any.stop_reason == "tool_use"
    calc_tool = next(block for block in response_any.content if block.type == "tool_use")
    
    # Complete the flow with tool result
    messages_any.append({"role": response_any.role, "content": response_any.content})
    messages_any.append({
        "role": "user",
        "content": [{
            "type": "tool_result",
            "tool_use_id": calc_tool.id,
            "content": "42"
        }]
    })
    
    response_any2 = await anthropic_client.messages.create(
        model="claude-sonnet-4-0",
        max_tokens=200,
        messages=messages_any,
        tools=[CALCULATOR_TOOL]
    )
    
    # Should give final answer
    assert "42" in response_any2.content[0].text