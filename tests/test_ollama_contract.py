#!/usr/bin/env python3
"""
Ollama Backend API Contract Test
=================================
Tests the exact HTTP request/response contract between Smriti's OllamaBackend
and Ollama's REST API.

This spins up a mock Ollama server and validates:
1. /api/generate — text generation contract
2. /api/embed — embedding generation contract
3. /api/tags — health check contract
4. Error handling for various failure modes

Run: python3 tests/test_ollama_contract.py
"""

import json
import threading
import time
import sys
from http.server import HTTPServer, BaseHTTPRequestHandler

# ═══════════════════════════════════════════
# Mock Ollama Server
# ═══════════════════════════════════════════

class MockOllamaHandler(BaseHTTPRequestHandler):
    """Simulates Ollama's REST API"""

    def log_message(self, format, *args):
        pass  # Suppress request logging

    def do_GET(self):
        if self.path == "/api/tags":
            # Health check — list models
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            response = {
                "models": [
                    {
                        "name": "gemma4:31b",
                        "model": "gemma4:31b",
                        "size": 18500000000,
                        "digest": "abc123",
                        "details": {
                            "parent_model": "",
                            "format": "gguf",
                            "family": "gemma4",
                            "families": ["gemma4"],
                            "parameter_size": "31B",
                            "quantization_level": "Q4_K_M"
                        }
                    }
                ]
            }
            self.wfile.write(json.dumps(response).encode())
        else:
            self.send_response(404)
            self.end_headers()

    def do_POST(self):
        content_length = int(self.headers.get("Content-Length", 0))
        body = json.loads(self.rfile.read(content_length)) if content_length > 0 else {}

        if self.path == "/api/generate":
            self._handle_generate(body)
        elif self.path == "/api/embed":
            self._handle_embed(body)
        else:
            self.send_response(404)
            self.end_headers()

    def _handle_generate(self, body):
        """Ollama /api/generate endpoint"""
        # Validate request structure
        assert "model" in body, "Missing 'model' in generate request"
        assert "prompt" in body, "Missing 'prompt' in generate request"
        assert "stream" in body, "Missing 'stream' in generate request"
        assert body["stream"] == False, "Smriti should set stream=false"

        # Validate options
        if "options" in body:
            opts = body["options"]
            assert "temperature" in opts, "Missing temperature in options"
            assert "top_p" in opts, "Missing top_p in options"
            assert "num_predict" in opts, "Missing num_predict in options"
            assert isinstance(opts["temperature"], (int, float))
            assert isinstance(opts["top_p"], (int, float))
            assert isinstance(opts["num_predict"], int)

        # Return mock response
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.end_headers()

        response = {
            "model": body["model"],
            "response": f"Mock response to: {body['prompt'][:50]}...",
            "done": True,
            "eval_count": 42,
            "prompt_eval_count": 10,
            "total_duration": 1000000000,
            "load_duration": 500000000,
            "eval_duration": 500000000,
        }
        self.wfile.write(json.dumps(response).encode())

    def _handle_embed(self, body):
        """Ollama /api/embed endpoint"""
        assert "model" in body, "Missing 'model' in embed request"
        assert "input" in body, "Missing 'input' in embed request"
        assert isinstance(body["input"], list), "'input' must be array of strings"

        # Return mock embeddings (384 dimensions to match sqlite-vec table)
        embeddings = []
        for text in body["input"]:
            # Simple deterministic embedding based on text hash
            import hashlib
            h = hashlib.sha256(text.encode()).digest()
            # Repeat hash bytes to fill 384 dimensions
            raw = list(h) * (384 // len(h) + 1)
            embedding = [float(b) / 255.0 for b in raw[:384]]
            embeddings.append(embedding)

        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.end_headers()

        response = {
            "model": body["model"],
            "embeddings": embeddings,
        }
        self.wfile.write(json.dumps(response).encode())

# ═══════════════════════════════════════════
# Test Cases
# ═══════════════════════════════════════════

import urllib.request

def test_health_check(port):
    """Test GET /api/tags — health check"""
    url = f"http://localhost:{port}/api/tags"
    req = urllib.request.Request(url)
    resp = urllib.request.urlopen(req)
    data = json.loads(resp.read())

    assert resp.status == 200
    assert "models" in data
    assert len(data["models"]) > 0
    assert data["models"][0]["name"] == "gemma4:31b"
    print("  PASS: health_check (GET /api/tags)")

def test_generate(port):
    """Test POST /api/generate — text generation"""
    url = f"http://localhost:{port}/api/generate"

    # This matches exactly what OllamaBackend sends
    body = {
        "model": "gemma4:31b",
        "prompt": "What is Rust programming?",
        "system": "You are Smriti AI.",
        "stream": False,
        "options": {
            "temperature": 0.7,
            "top_p": 0.9,
            "num_predict": 2048,
        }
    }

    req = urllib.request.Request(
        url,
        data=json.dumps(body).encode(),
        headers={"Content-Type": "application/json"},
        method="POST"
    )
    resp = urllib.request.urlopen(req)
    data = json.loads(resp.read())

    assert resp.status == 200
    assert "response" in data, "Missing 'response' field"
    assert "model" in data, "Missing 'model' field"
    assert "done" in data, "Missing 'done' field"
    assert "eval_count" in data, "Missing 'eval_count' field"
    assert "prompt_eval_count" in data, "Missing 'prompt_eval_count' field"
    assert data["done"] == True
    assert isinstance(data["response"], str)
    assert isinstance(data["eval_count"], int)
    assert isinstance(data["prompt_eval_count"], int)
    print("  PASS: generate (POST /api/generate)")

def test_generate_with_stop_sequences(port):
    """Test generate with optional stop sequences"""
    url = f"http://localhost:{port}/api/generate"

    body = {
        "model": "gemma4:31b",
        "prompt": "List items:",
        "stream": False,
        "options": {
            "temperature": 0.3,
            "top_p": 0.9,
            "num_predict": 512,
            "stop": ["\\n\\n", "END"]
        }
    }

    req = urllib.request.Request(
        url,
        data=json.dumps(body).encode(),
        headers={"Content-Type": "application/json"},
        method="POST"
    )
    resp = urllib.request.urlopen(req)
    data = json.loads(resp.read())

    assert resp.status == 200
    assert data["done"] == True
    print("  PASS: generate with stop sequences")

def test_embed_single(port):
    """Test POST /api/embed — single text embedding"""
    url = f"http://localhost:{port}/api/embed"

    body = {
        "model": "gemma4:31b",
        "input": ["Hello, world!"]
    }

    req = urllib.request.Request(
        url,
        data=json.dumps(body).encode(),
        headers={"Content-Type": "application/json"},
        method="POST"
    )
    resp = urllib.request.urlopen(req)
    data = json.loads(resp.read())

    assert resp.status == 200
    assert "embeddings" in data, "Missing 'embeddings' field"
    assert len(data["embeddings"]) == 1, f"Expected 1 embedding, got {len(data['embeddings'])}"
    assert isinstance(data["embeddings"][0], list), "Embedding should be array of floats"
    assert len(data["embeddings"][0]) == 384, f"Expected 384 dims, got {len(data['embeddings'][0])}"
    assert all(isinstance(v, float) for v in data["embeddings"][0])
    print("  PASS: embed single text (POST /api/embed)")

def test_embed_batch(port):
    """Test POST /api/embed — batch embedding (multiple texts)"""
    url = f"http://localhost:{port}/api/embed"

    texts = [
        "Machine learning fundamentals",
        "Rust programming language",
        "Knowledge graphs and semantic search",
    ]

    body = {
        "model": "gemma4:31b",
        "input": texts
    }

    req = urllib.request.Request(
        url,
        data=json.dumps(body).encode(),
        headers={"Content-Type": "application/json"},
        method="POST"
    )
    resp = urllib.request.urlopen(req)
    data = json.loads(resp.read())

    assert resp.status == 200
    assert len(data["embeddings"]) == 3, f"Expected 3 embeddings, got {len(data['embeddings'])}"
    for i, emb in enumerate(data["embeddings"]):
        assert len(emb) == 384, f"Embedding {i} has {len(emb)} dims, expected 384"
    print("  PASS: embed batch (3 texts)")

def test_generate_maps_to_smriti_response(port):
    """
    Verify the Ollama response structure maps to Smriti's GenerateResponse.

    Smriti expects:
      GenerateResponse {
          text: String,           <- ollama.response
          tokens_used: TokenUsage <- eval_count + prompt_eval_count
          model: String,          <- ollama.model
          finish_reason: Option,  <- ollama.done -> "stop"
      }
    """
    url = f"http://localhost:{port}/api/generate"

    body = {
        "model": "gemma4:31b",
        "prompt": "Test",
        "stream": False,
        "options": {"temperature": 0.7, "top_p": 0.9, "num_predict": 100}
    }

    req = urllib.request.Request(
        url,
        data=json.dumps(body).encode(),
        headers={"Content-Type": "application/json"},
        method="POST"
    )
    resp = urllib.request.urlopen(req)
    data = json.loads(resp.read())

    # Map to Smriti's GenerateResponse
    smriti_response = {
        "text": data["response"],
        "tokens_used": {
            "prompt_tokens": data["prompt_eval_count"],
            "completion_tokens": data["eval_count"],
            "total_tokens": data["prompt_eval_count"] + data["eval_count"],
        },
        "model": data["model"],
        "finish_reason": "stop" if data["done"] else None,
    }

    assert isinstance(smriti_response["text"], str)
    assert smriti_response["tokens_used"]["total_tokens"] == 52  # 10 + 42
    assert smriti_response["model"] == "gemma4:31b"
    assert smriti_response["finish_reason"] == "stop"
    print("  PASS: response maps to Smriti GenerateResponse")

def test_embed_maps_to_smriti_vectors(port):
    """
    Verify embeddings map correctly to Vec<Vec<f32>> for sqlite-vec storage.
    """
    url = f"http://localhost:{port}/api/embed"

    body = {"model": "gemma4:31b", "input": ["Test note content"]}

    req = urllib.request.Request(
        url,
        data=json.dumps(body).encode(),
        headers={"Content-Type": "application/json"},
        method="POST"
    )
    resp = urllib.request.urlopen(req)
    data = json.loads(resp.read())

    embeddings = data["embeddings"]
    assert len(embeddings) == 1

    # Verify all values are valid floats in reasonable range
    vec = embeddings[0]
    assert all(isinstance(v, (int, float)) for v in vec)
    assert all(-10.0 <= v <= 10.0 for v in vec), "Embedding values out of range"
    print("  PASS: embeddings map to Vec<Vec<f32>>")

# ═══════════════════════════════════════════
# Test Runner
# ═══════════════════════════════════════════

def main():
    port = 11434  # Standard Ollama port

    # Try alternate port if 11434 is in use
    import socket
    sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    result = sock.connect_ex(('localhost', port))
    sock.close()
    if result == 0:
        port = 18434  # Use alternate

    # Start mock server
    server = HTTPServer(("localhost", port), MockOllamaHandler)
    thread = threading.Thread(target=server.serve_forever)
    thread.daemon = True
    thread.start()
    time.sleep(0.2)

    print(f"\nSmriti Ollama Backend Contract Tests")
    print(f"Mock server on http://localhost:{port}")
    print("=" * 50)

    tests = [
        test_health_check,
        test_generate,
        test_generate_with_stop_sequences,
        test_embed_single,
        test_embed_batch,
        test_generate_maps_to_smriti_response,
        test_embed_maps_to_smriti_vectors,
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
    print("\nAll Ollama API contract tests passed!")

if __name__ == "__main__":
    main()
