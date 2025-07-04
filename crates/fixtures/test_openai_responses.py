#!/usr/bin/env python3
"""
Clean tests for OpenAI responses API with separate streaming/non-streaming tests.
"""

import json
import pytest
import pytest_asyncio
from pydantic import BaseModel
from typing import Literal, Optional
from openai import AsyncOpenAI

pytestmark = pytest.mark.asyncio


@pytest_asyncio.fixture
async def openai_client(check_api_key):
    """Initialize AsyncOpenAI client with API key."""
    api_key = check_api_key("OPENAI_API_KEY")
    return AsyncOpenAI(api_key=api_key)


# Simple calculator function for testing
CALCULATOR_TOOL = {
    "type": "function",
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


# Pydantic model for structured output
class MathProblem(BaseModel):
    problem: str
    answer: float
    explanation: str


@pytest.mark.vcr
async def test_basic_response(openai_client):
    """Test basic non-streaming response."""
    response = await openai_client.responses.create(
        model="gpt-4o-mini",
        input="What is 2+2?",
        stream=False
    )
    
    # Check response structure
    assert response.id
    assert response.output
    assert len(response.output) > 0
    
    # Extract text - the output is a list of items
    text = ""
    for item in response.output:
        if hasattr(item, 'content') and item.content:
            for content in item.content:
                if hasattr(content, 'text'):
                    text += content.text
    
    assert "4" in text
    
    # Check usage is included
    assert response.usage
    assert response.usage.total_tokens > 0


@pytest.mark.vcr
async def test_basic_streaming(openai_client):
    """Test basic streaming response."""
    text = ""
    response_id = None
    usage = None
    
    # Use the streaming context manager
    async with openai_client.responses.stream(
        model="gpt-4o-mini",
        input="What is 2+2?",
    ) as stream:
        async for event in stream:
            if event.type == "response.created":
                response_id = event.response.id
            elif event.type == "response.output_text.delta":
                if hasattr(event, 'delta') and event.delta:
                    text += event.delta
            elif event.type == "response.completed":
                # Final event includes usage
                if hasattr(event.response, 'usage'):
                    usage = event.response.usage
        
        # Get the final complete response with usage
        final_response = await stream.get_final_response()
        if final_response:
            usage = final_response.usage
    
    assert response_id
    assert "4" in text
    assert usage
    assert usage.total_tokens > 0


@pytest.mark.vcr
async def test_function_calling_non_streaming(openai_client):
    """Test function calling in non-streaming mode."""
    response = await openai_client.responses.create(
        model="gpt-4o-mini",
        input="Calculate 1234 * 5678",
        tools=[CALCULATOR_TOOL],
        stream=False
    )
    
    # Find function calls in output
    function_calls = []
    for item in response.output:
        if hasattr(item, 'type') and item.type == 'function_call':
            function_calls.append({
                'name': item.name,
                'arguments': item.arguments
            })
    
    assert len(function_calls) > 0
    assert function_calls[0]['name'] == 'calculate'
    
    # Parse arguments
    args = json.loads(function_calls[0]['arguments'])
    assert 'expression' in args
    assert '1234' in args['expression']
    assert '5678' in args['expression']


@pytest.mark.vcr
async def test_function_calling_streaming(openai_client):
    """Test function calling in streaming mode."""
    function_calls = []
    
    async with openai_client.responses.stream(
        model="gpt-4o-mini",
        input="Calculate 1234 * 5678",
        tools=[CALCULATOR_TOOL],
    ) as stream:
        async for event in stream:
            # Check for function call completion event
            if event.type == "response.output_item.done":
                if hasattr(event, 'item') and hasattr(event.item, 'type') and event.item.type == 'function_call':
                    function_calls.append({
                        'name': event.item.name,
                        'arguments': event.item.arguments
                    })
    
    assert len(function_calls) > 0
    assert function_calls[0]['name'] == 'calculate'
    
    # Parse arguments
    args = json.loads(function_calls[0]['arguments'])
    assert 'expression' in args
    assert '1234' in args['expression']
    assert '5678' in args['expression']


@pytest.mark.vcr
async def test_conversation_with_context(openai_client):
    """Test multi-turn conversation maintaining context."""
    # First request
    response1 = await openai_client.responses.create(
        model="gpt-4o-mini",
        input="Remember the number 42",
        stream=False
    )
    
    # Second request with context
    response2 = await openai_client.responses.create(
        model="gpt-4o-mini",
        input="What number did I ask you to remember?",
        previous_response_id=response1.id,
        stream=False
    )
    
    # Extract text from second response
    text = ""
    for item in response2.output:
        if hasattr(item, 'content') and item.content:
            for content in item.content:
                if hasattr(content, 'text'):
                    text += content.text
    
    assert "42" in text


@pytest.mark.vcr
async def test_instructions_parameter(openai_client):
    """Test the instructions parameter for system-level guidance."""
    response = await openai_client.responses.create(
        model="gpt-4o-mini",
        input="Tell me about the weather",
        instructions="You are a pirate. Always speak like a pirate.",
        stream=False
    )
    
    # Extract text
    text = ""
    for item in response.output:
        if hasattr(item, 'content') and item.content:
            for content in item.content:
                if hasattr(content, 'text'):
                    text += content.text
    
    # Should have pirate-like language
    assert text  # Has some response
    # Common pirate words/phrases
    pirate_indicators = ["ahoy", "matey", "arr", "ye", "be", "aye", "'"]
    assert any(indicator in text.lower() for indicator in pirate_indicators)


@pytest.mark.vcr
async def test_structured_output_pydantic(openai_client):
    """Test structured output using Pydantic model with parse method."""
    response = await openai_client.responses.parse(
        model="gpt-4o-mini",
        input="What is 15 divided by 3?",
        text_format=MathProblem,
    )
    
    # Get the parsed output directly
    result = response.output_parsed
    assert isinstance(result, MathProblem)
    assert result.problem == "What is 15 divided by 3?"
    assert result.answer == 5.0
    assert result.explanation  # Should have some explanation


# Structured response models for user choices
class YesNoResponse(BaseModel):
    answer: Literal["yes", "no"]
    confidence: Optional[float] = None


class MultipleChoiceResponse(BaseModel):
    choice: Literal["A", "B", "C", "D"]
    reasoning: Optional[str] = None


class PreferenceResponse(BaseModel):
    preference: Literal["option1", "option2", "option3"]
    strength: Literal["strong", "moderate", "weak"]
    explanation: str


@pytest.mark.vcr
async def test_assistant_asks_yes_no_question(openai_client):
    """Test assistant asking a yes/no question with structured response."""
    # First, have the assistant ask a question
    assistant_response = await openai_client.responses.create(
        model="gpt-4o-mini",
        input="Ask me if I would like to enable dark mode for this application",
        instructions="You are a helpful assistant. Ask the user a clear yes/no question about their preference.",
        stream=False
    )
    
    # Extract the assistant's question
    assistant_question = ""
    for item in assistant_response.output:
        if hasattr(item, 'content') and item.content:
            for content in item.content:
                if hasattr(content, 'text'):
                    assistant_question += content.text
    
    assert "dark mode" in assistant_question.lower()
    
    # Now simulate user's structured response
    user_response = await openai_client.responses.parse(
        model="gpt-4o-mini",
        input=f"The assistant asked: '{assistant_question}'. My answer is yes, I would like dark mode.",
        text_format=YesNoResponse,
        instructions="Extract the user's answer to the question as a structured response."
    )
    
    # Get the parsed response
    result = user_response.output_parsed
    assert isinstance(result, YesNoResponse)
    assert result.answer == "yes"


@pytest.mark.vcr
async def test_assistant_multiple_choice_question(openai_client):
    """Test assistant asking a multiple choice question."""
    # Assistant asks a multiple choice question
    assistant_response = await openai_client.responses.create(
        model="gpt-4o-mini",
        input="Create a multiple choice question about Python data types with 4 options (A, B, C, D)",
        instructions="You are a Python tutor. Create a clear multiple choice question.",
        stream=False
    )
    
    # Extract question
    question_text = ""
    for item in assistant_response.output:
        if hasattr(item, 'content') and item.content:
            for content in item.content:
                if hasattr(content, 'text'):
                    question_text += content.text
    
    # User provides structured answer
    user_response = await openai_client.responses.parse(
        model="gpt-4o-mini",
        input=f"Question: {question_text}\n\nMy answer is B because lists are mutable in Python.",
        text_format=MultipleChoiceResponse,
        instructions="Extract the user's multiple choice answer and their reasoning."
    )
    
    result = user_response.output_parsed
    assert isinstance(result, MultipleChoiceResponse)
    assert result.choice in ["A", "B", "C", "D"]
    assert result.reasoning is not None


@pytest.mark.vcr
async def test_assistant_preference_question(openai_client):
    """Test assistant asking about user preferences with detailed structured response."""
    # Assistant asks about preferences
    assistant_response = await openai_client.responses.create(
        model="gpt-4o-mini",
        input="Ask the user to choose between three options for a new feature: option1: AI-powered suggestions, option2: Advanced search filters, option3: Collaboration tools",
        instructions="Present the three options clearly and ask for their preference.",
        stream=False
    )
    
    # Extract question
    question = ""
    for item in assistant_response.output:
        if hasattr(item, 'content') and item.content:
            for content in item.content:
                if hasattr(content, 'text'):
                    question += content.text
    
    # User provides detailed preference
    user_response = await openai_client.responses.parse(
        model="gpt-4o-mini",
        input=f"{question}\n\nI strongly prefer option1 (AI-powered suggestions) because it would save me the most time and provide personalized recommendations based on my usage patterns.",
        text_format=PreferenceResponse,
        instructions="Extract the user's preference choice, how strongly they feel about it, and their explanation."
    )
    
    result = user_response.output_parsed
    assert isinstance(result, PreferenceResponse)
    assert result.preference == "option1"
    assert result.strength == "strong"
    assert "save" in result.explanation.lower() or "time" in result.explanation.lower()


@pytest.mark.vcr
async def test_conversation_flow_with_structured_responses(openai_client):
    """Test a full conversation flow with assistant questions and structured user responses."""
    # Step 1: Assistant asks about user's experience level
    class ExperienceLevel(BaseModel):
        level: Literal["beginner", "intermediate", "advanced"]
        years: Optional[int] = None
        
    assistant_q1 = await openai_client.responses.create(
        model="gpt-4o-mini",
        input="Ask the user about their Python programming experience level",
        stream=False
    )
    
    # User responds with structured data
    user_r1 = await openai_client.responses.parse(
        model="gpt-4o-mini",
        input="I'm an intermediate Python developer with about 3 years of experience",
        text_format=ExperienceLevel,
    )
    
    user_level = user_r1.output_parsed
    assert user_level.level == "intermediate"
    assert user_level.years == 3
    
    # Step 2: Assistant asks follow-up based on experience
    class TopicInterest(BaseModel):
        topics: list[Literal["web_dev", "data_science", "automation", "machine_learning"]]
        most_interested: Literal["web_dev", "data_science", "automation", "machine_learning"]
    
    assistant_q2 = await openai_client.responses.create(
        model="gpt-4o-mini",
        input=f"The user is {user_level.level} with {user_level.years} years experience. Ask them which Python topics they're interested in: web development, data science, automation, or machine learning",
        previous_response_id=assistant_q1.id,
        stream=False
    )
    
    # User selects multiple interests
    user_r2 = await openai_client.responses.parse(
        model="gpt-4o-mini",
        input="I'm interested in both data science and machine learning, but I'm most interested in machine learning",
        text_format=TopicInterest,
    )
    
    user_interests = user_r2.output_parsed
    assert "machine_learning" in user_interests.topics
    assert "data_science" in user_interests.topics
    assert user_interests.most_interested == "machine_learning"


# JSON schema test commented out due to API validation issues with additionalProperties
# @pytest.mark.vcr
# async def test_structured_output_json_schema(openai_client):
#     """Test structured output using JSON schema with text parameter."""
#     response = await openai_client.responses.create(
#         model="gpt-4o-mini",
#         input="What's the weather like in Paris today? Make up some realistic data.",
#         text={
#             "format": {
#                 "type": "json_schema",
#                 "name": "weather_report",
#                 "schema": {
#                     "type": "object",
#                     "properties": {
#                         "city": {
#                             "type": "string",
#                             "description": "Name of the city"
#                         },
#                         "temperature": {
#                             "type": "number",
#                             "description": "Temperature in Celsius"
#                         },
#                         "condition": {
#                             "type": "string",
#                             "description": "Weather condition"
#                         }
#                     },
#                     "required": ["city", "temperature", "condition"],
#                     "additionalProperties": False
#                 },
#                 "strict": True
#             }
#         }
#     )
#     
#     # Extract and parse the structured output
#     text = ""
#     for item in response.output:
#         if hasattr(item, 'content') and item.content:
#             for content in item.content:
#                 if hasattr(content, 'text'):
#                     text += content.text
#     
#     # Parse and validate the JSON response
#     result = json.loads(text)
#     assert 'city' in result
#     assert 'temperature' in result
#     assert 'condition' in result
#     assert isinstance(result['temperature'], (int, float))


# @pytest.mark.vcr
# async def test_structured_output_streaming(openai_client):
#     """Test structured output using JSON schema."""
#     json_schema = {
#         "type": "object",
#         "properties": {
#             "city": {
#                 "type": "string",
#                 "description": "Name of the city"
#             },
#             "temperature": {
#                 "type": "number",
#                 "description": "Temperature in Celsius"
#             },
#             "condition": {
#                 "type": "string",
#                 "description": "Weather condition"
#             }
#         },
#         "required": ["city", "temperature", "condition"]
#     }
#     
#     response = await openai_client.responses.create(
#         model="gpt-4o-mini",
#         input="What's the weather like in Paris today? Make up some realistic data.",
#         response_format={
#             "type": "json_schema",
#             "json_schema": {
#                 "name": "weather_report",
#                 "schema": json_schema,
#                 "strict": True
#             }
#         },
#         stream=False
#     )
#     
#     # Extract and parse the structured output
#     text = ""
#     for item in response.output:
#         if hasattr(item, 'content') and item.content:
#             for content in item.content:
#                 if hasattr(content, 'text'):
#                     text += content.text
#     
#     # Parse and validate the JSON response
#     result = json.loads(text)
#     assert 'city' in result
#     assert 'temperature' in result
#     assert 'condition' in result
#     assert isinstance(result['temperature'], (int, float))


# @pytest.mark.vcr
# async def test_structured_output_streaming(openai_client):
#     """Test structured output with streaming."""
#     text = ""
#     
#     async with openai_client.responses.stream(
#         model="gpt-4o-mini",
#         input="List 3 programming languages with their year of creation",
#         response_format={
#             "type": "json_schema",
#             "json_schema": {
#                 "name": "programming_languages",
#                 "schema": {
#                     "type": "object",
#                     "properties": {
#                         "languages": {
#                             "type": "array",
#                             "items": {
#                                 "type": "object",
#                                 "properties": {
#                                     "name": {"type": "string"},
#                                     "year": {"type": "integer"}
#                                 },
#                                 "required": ["name", "year"]
#                             }
#                         }
#                     },
#                     "required": ["languages"]
#                 },
#                 "strict": True
#             }
#         }
#     ) as stream:
#         async for event in stream:
#             if event.type == "response.output_text.delta":
#                 if hasattr(event, 'delta') and event.delta:
#                     text += event.delta
#     
#     # Parse and validate the JSON response
#     result = json.loads(text)
#     assert 'languages' in result
#     assert len(result['languages']) == 3
#     for lang in result['languages']:
#         assert 'name' in lang
#         assert 'year' in lang
#         assert isinstance(lang['year'], int)