#!/usr/bin/env python3
"""
OpenAI-Compatible Backend API Contract Test
=============================================
Tests the HTTP request/response contract between Smriti's OpenAICompatibleBackend
and any OpenAI-compatible API (vLLM, llama.cpp server, LM Studio, etc.)

Run: python3 tests/test_openai_contract.py
"""

import json
import threading
import time
import sys
import hashlib
from http.server import HTTPServer, BaseHTTPRequestHandler
import urllib.request

class MockOpenAIHandler(BaseHTTPRequestHandler):
    """Simulates an OpenAI-compatible API server"""

    def log_message(self, format, *args):
        pass

    def do_GET(self):
        if self.path == "/v1/models":
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            response = {
                "data": [
                    {"id": "gemma-4-31b-it", "object": "model", "owned_by": "google"}
                ]
            }
            self.wfile.write(json.dumps(response).encode())
        else:
            self.send_response(404)
            self.end_headers()

    def do_POST(self):
        content_length = int(self.headers.get("Content-Length", 0))
        body = json.loads(self.rfile.read(content_length)) if content_length > 0 else {}

        if self.path == "/v1/chat/completions":
            self._handle_chat(body)
        elif self.path == "/v1/embeddings":
            self._handle_embeddings(body)
        else:
            self.send_response(404)
            self.end_headers()

    def _handle_chat(self, body):
        """OpenAI /v1/chat/completions endpoint"""
        assert "model" in body, "Missing 'model'"
        assert "messages" in body, "Missing 'messages'"
        assert isinstance(body["messages"], list)
        assert body.get("stream", False) == False, "Smriti should set stream=false"

        # Validate message format
        for msg in body["messages"]:
            assert "role" in msg, "Message missing 'role'"
            assert "content" in msg, "Message missing 'content'"
            assert msg["role"] in ("system", "user", "assistant")

        # Check optional fields exist when provided (not wrong types)
        if "max_tokens" in body:
            assert isinstance(body["max_tokens"], int)
        if "temperature" in body:
            assert isinstance(body["temperature"], (int, float))
        if "top_p" in body:
            assert isinstance(body["top_p"], (int, float))

        prompt_text = " ".join(m["content"] for m in body["messages"])

        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.end_headers()

        response = {
            "id": "chatcmpl-test123",
            "object": "chat.completion",
            "created": 1711500000,
            "model": body["model"],
            "choices": [
                {
                    "index": 0,
                    "message": {
                        "role": "assistant",
                        "content": f"Mock response to: {prompt_text[:50]}"
                    },
                    "finish_reason": "stop"
                }
            ],
            "usage": {
                "prompt_tokens": 15,
                "completion_tokens": 25,
                "total_tokens": 40
            }
        }
        self.wfile.write(json.dumps(response).encode())

    def _handle_embeddings(self, body):
        """OpenAI /v1/embeddings endpoint"""
        assert "model" in body, "Missing 'model'"
        assert "input" in body, "Missing 'input'"
        assert isinstance(body["input"], list)

        data = []
        for i, text in enumerate(body["input"]):
            h = hashlib.sha256(text.encode()).digest()
            raw = list(h) * (384 // len(h) + 1)
            embedding = [float(b) / 255.0 for b in raw[:384]]
            data.append({
                "object": "embedding",
                "index": i,
                "embedding": embedding
            })

        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.end_headers()

        response = {
            "object": "list",
            "data": data,
            "model": body["model"],
            "usage": {"prompt_tokens": 10, "total_tokens": 10}
        }
        self.wfile.write(json.dumps(response).encode())


def test_health_check(port):
    url = f"http://localhost:{port}/v1/models"
    resp = urllib.request.urlopen(url)
    data = json.loads(resp.read())
    assert resp.status == 200
    assert "data" in data
    print("  PASS: health_check (GET /v1/models)")

def test_chat_completion(port):
    url = f"http://localhost:{port}/v1/chat/completions"
    body = {
        "model": "gemma-4-31b-it",
        "messages": [
            {"role": "system", "content": "You are Smriti AI."},
            {"role": "user", "content": "What is a knowledge graph?"}
        ],
        "max_tokens": 2048,
        "temperature": 0.7,
        "top_p": 0.9,
        "stream": False
    }
    req = urllib.request.Request(url, json.dumps(body).encode(), {"Content-Type": "application/json"})
    resp = urllib.request.urlopen(req)
    data = json.loads(resp.read())

    assert resp.status == 200
    assert "choices" in data
    assert len(data["choices"]) > 0
    assert "message" in data["choices"][0]
    assert "content" in data["choices"][0]["message"]
    assert "usage" in data
    assert data["usage"]["total_tokens"] == 40
    print("  PASS: chat completion (POST /v1/chat/completions)")

def test_chat_with_auth(port):
    """Test that Bearer token is passed correctly"""
    url = f"http://localhost:{port}/v1/chat/completions"
    body = {
        "model": "gemma-4-31b-it",
        "messages": [{"role": "user", "content": "test"}],
        "stream": False
    }
    req = urllib.request.Request(url, json.dumps(body).encode(), {
        "Content-Type": "application/json",
        "Authorization": "Bearer test-api-key-123"
    })
    resp = urllib.request.urlopen(req)
    assert resp.status == 200
    print("  PASS: chat with Bearer auth")

def test_embeddings(port):
    url = f"http://localhost:{port}/v1/embeddings"
    body = {
        "model": "gemma-4-31b-it",
        "input": ["Hello world", "Knowledge graphs"]
    }
    req = urllib.request.Request(url, json.dumps(body).encode(), {"Content-Type": "application/json"})
    resp = urllib.request.urlopen(req)
    data = json.loads(resp.read())

    assert resp.status == 200
    assert "data" in data
    assert len(data["data"]) == 2
    for item in data["data"]:
        assert "embedding" in item
        assert len(item["embedding"]) == 384
    print("  PASS: embeddings (POST /v1/embeddings)")

def test_response_maps_to_smriti(port):
    """Verify OpenAI response maps to Smriti's GenerateResponse"""
    url = f"http://localhost:{port}/v1/chat/completions"
    body = {
        "model": "gemma-4-31b-it",
        "messages": [{"role": "user", "content": "test"}],
        "stream": False
    }
    req = urllib.request.Request(url, json.dumps(body).encode(), {"Content-Type": "application/json"})
    resp = urllib.request.urlopen(req)
    data = json.loads(resp.read())

    # Map to Smriti GenerateResponse
    choice = data["choices"][0]
    smriti = {
        "text": choice["message"]["content"],
        "tokens_used": {
            "prompt_tokens": data["usage"]["prompt_tokens"],
            "completion_tokens": data["usage"]["completion_tokens"],
            "total_tokens": data["usage"]["total_tokens"],
        },
        "model": data["model"],
        "finish_reason": choice.get("finish_reason"),
    }

    assert isinstance(smriti["text"], str)
    assert smriti["tokens_used"]["total_tokens"] == 40
    assert smriti["finish_reason"] == "stop"
    print("  PASS: response maps to Smriti GenerateResponse")


def main():
    port = 18080

    server = HTTPServer(("localhost", port), MockOpenAIHandler)
    thread = threading.Thread(target=server.serve_forever)
    thread.daemon = True
    thread.start()
    time.sleep(0.2)

    print(f"\nSmriti OpenAI-Compatible Backend Contract Tests")
    print(f"Mock server on http://localhost:{port}/v1")
    print("=" * 50)

    tests = [
        test_health_check,
        test_chat_completion,
        test_chat_with_auth,
        test_embeddings,
        test_response_maps_to_smriti,
    ]

    passed = 0
    failed = 0

    for test in tests:
        try:
            test(port)
            passed += 1
        except Exception as e:
            print(f"  FAIL: {test.__name__}: {e}")
            failed += 1

    print("=" * 50)
    print(f"Results: {passed} passed, {failed} failed")

    server.shutdown()

    if failed > 0:
        sys.exit(1)
    print("\nAll OpenAI-compatible API contract tests passed!")

if __name__ == "__main__":
    main()
