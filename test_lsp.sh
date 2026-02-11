#!/bin/bash

# Test script for PHPantomLSP
# Sends LSP JSON-RPC messages over stdin and prints responses from stdout

cargo build 2>/dev/null

PROJECT_DIR="$PWD"
FILE_URI="file://${PROJECT_DIR}/example.php"

# Function to send an LSP message with correct Content-Length
send() {
    local body="$1"
    local len=${#body}
    printf "Content-Length: %d\r\n\r\n%s" "$len" "$body"
}

# Read and JSON-encode the file content safely
EXAMPLE_PHP_ESCAPED=$(python3 -c '
import json, sys
with open("example.php", "r") as f:
    print(json.dumps(f.read()))
')

{
    # 1. Initialize
    send '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"capabilities":{},"processId":null,"rootUri":null,"workspaceFolders":null}}'
    sleep 1

    # 2. Initialized notification (no id = notification, no response expected)
    send '{"jsonrpc":"2.0","method":"initialized","params":{}}'
    sleep 0.5

    # 3. Open example.php from the project root
    send "{\"jsonrpc\":\"2.0\",\"method\":\"textDocument/didOpen\",\"params\":{\"textDocument\":{\"uri\":\"${FILE_URI}\",\"languageId\":\"php\",\"version\":1,\"text\":${EXAMPLE_PHP_ESCAPED}}}}"
    sleep 0.5

    # 4. Hover over "PHPantom" on line 20 (comment: "// PHPantom provides basic LSP functionality")
    send "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"textDocument/hover\",\"params\":{\"textDocument\":{\"uri\":\"${FILE_URI}\"},\"position\":{\"line\":20,\"character\":11}}}"
    sleep 1

    # 5. Hover over a word that is NOT "PHPantom" (should return null) - "echo" on line 23
    send "{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"textDocument/hover\",\"params\":{\"textDocument\":{\"uri\":\"${FILE_URI}\"},\"position\":{\"line\":23,\"character\":9}}}"
    sleep 1

    # 6. Shutdown
    send '{"jsonrpc":"2.0","id":4,"method":"shutdown"}'
    sleep 0.5

    # 7. Exit
    send '{"jsonrpc":"2.0","method":"exit","params":{}}'
} | cargo run 2>/dev/null