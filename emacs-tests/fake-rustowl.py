"""Small stdio LSP fixture for the Emacs transport integration test."""
import json
import sys

opened = set()
initialized = False
analyzed = False
while True:
    headers = {}
    while line := sys.stdin.buffer.readline():
        if line == b"\r\n":
            break
        key, value = line.decode().split(":", 1)
        headers[key.lower()] = value.strip()
    if not headers:
        break
    message = json.loads(sys.stdin.buffer.read(int(headers["content-length"])))
    method = message["method"]
    params = message.get("params")
    result = None
    error = None
    if method == "initialize":
        result = {"capabilities": {"textDocumentSync": 2}}
    elif method == "initialized":
        initialized = True
    elif method == "textDocument/didOpen":
        assert initialized
        opened.add(params["textDocument"]["uri"])
    elif method == "textDocument/didClose":
        opened.remove(params["textDocument"]["uri"])
    elif method == "textDocument/didChange":
        assert params["textDocument"]["uri"] in opened
        analyzed = False
    elif method == "rustowl/analyze":
        assert initialized
        analyzed = True
    elif method == "rustowl/cursor":
        assert initialized and params["document"]["uri"] in opened
        result = {"is_analyzed": analyzed, "decorations": [{
            "type": "imm_borrow", "overlapped": False,
            "range": {"start": {"line": 0, "character": 0},
                      "end": {"line": 0, "character": 3}}
        }] if analyzed else []}
    elif method == "shutdown":
        assert not opened
    elif method == "exit":
        break
    else:
        error = {"code": -32601, "message": method}
    if "id" in message:
        response = {"jsonrpc": "2.0", "id": message["id"]}
        response.update({"error": error} if error else {"result": result})
        payload = json.dumps(response).encode()
        sys.stdout.buffer.write(f"Content-Length: {len(payload)}\r\n\r\n".encode() + payload)
        sys.stdout.buffer.flush()
