// Smoke-test the wasm LSP reactor under Node's WASI.
//
// Drives the module the same way a browser host does: instantiate once, then
// push LSP JSON-RPC messages through `lsp_handle`. Verifies that completion,
// hover, definition and signature help all come back with real results.
//
// Usage (see docs/wasm.md for the build command):
//   node scripts/wasm-smoke-test.mjs [path/to/phpantom_lsp.wasm]

import { WASI } from 'node:wasi';
import { readFileSync } from 'node:fs';
import { argv, exit } from 'node:process';

const wasmPath =
	argv[2] ?? 'target/wasm32-wasip1/wasm-release/phpantom_lsp.wasm';

const wasi = new WASI({ version: 'preview1', args: [], env: {} });

const bytes = readFileSync(wasmPath);
const module = await WebAssembly.compile(bytes);
const instance = await WebAssembly.instantiate(module, wasi.getImportObject());
wasi.initialize(instance);

const { memory, lsp_alloc, lsp_dealloc, lsp_handle, lsp_response_len } =
	instance.exports;

const encoder = new TextEncoder();
const decoder = new TextDecoder();

/** Send one LSP message; returns the parsed response, or null for notifications. */
function send(message) {
	const payload = encoder.encode(JSON.stringify(message));
	const inPtr = lsp_alloc(payload.length);
	new Uint8Array(memory.buffer, inPtr, payload.length).set(payload);

	const outPtr = lsp_handle(inPtr, payload.length);
	lsp_dealloc(inPtr, payload.length);
	if (outPtr === 0) return null;

	const len = lsp_response_len();
	// Copy before any further wasm call: growing linear memory detaches the
	// ArrayBuffer this view is built on.
	const out = new Uint8Array(memory.buffer, outPtr, len).slice();
	lsp_dealloc(outPtr, len);
	return JSON.parse(decoder.decode(out));
}

const uri = 'file:///try/index.php';
const source = `<?php

class Greeter
{
    public function __construct(private string $greeting) {}

    public function hello(string $name): string
    {
        return "{$this->greeting}, {$name}!";
    }
}

$g = new Greeter('Hello');
$g->hello('world');
`;

// `$g->` on the last line, at the column just after the arrow.
const lines = source.split('\n');
const callLine = lines.findIndex((l) => l.includes('$g->hello'));
const arrowCol = lines[callLine].indexOf('->') + 2;

let id = 0;
const request = (method, params) => send({ jsonrpc: '2.0', id: ++id, method, params });
const notify = (method, params) => send({ jsonrpc: '2.0', method, params });

const failures = [];
function check(name, ok, detail) {
	if (ok) {
		console.log(`  ok    ${name}`);
	} else {
		console.log(`  FAIL  ${name}: ${detail}`);
		failures.push(name);
	}
}

const init = request('initialize', { capabilities: {} });
check(
	'initialize advertises completion + hover',
	init?.result?.capabilities?.completionProvider != null &&
		init.result.capabilities.hoverProvider === true,
	JSON.stringify(init),
);

check(
	'initialize reports a server name and version',
	(init?.result?.serverInfo?.name ?? '') !== '' &&
		(init?.result?.serverInfo?.version ?? '') !== '',
	JSON.stringify(init?.result?.serverInfo),
);

notify('initialized', {});
notify('textDocument/didOpen', {
	textDocument: { uri, languageId: 'php', version: 1, text: source },
});

const completion = request('textDocument/completion', {
	textDocument: { uri },
	position: { line: callLine, character: arrowCol },
});
const items = completion?.result?.items ?? completion?.result ?? [];
check(
	'completion offers Greeter::hello',
	Array.isArray(items) &&
		items.some((i) => i.filterText === 'hello' || i.label.startsWith('hello')),
	`${items.length} items: ${JSON.stringify(items.slice(0, 5))}`,
);

const hover = request('textDocument/hover', {
	textDocument: { uri },
	position: { line: callLine, character: arrowCol + 2 },
});
const hoverText = JSON.stringify(hover?.result?.contents ?? '');
check(
	'hover describes hello()',
	hoverText.includes('hello'),
	hoverText,
);

const definition = request('textDocument/definition', {
	textDocument: { uri },
	position: { line: callLine, character: arrowCol + 2 },
});
check(
	'definition resolves to the declaration',
	definition?.result != null && JSON.stringify(definition.result).includes(uri),
	JSON.stringify(definition),
);

const highlight = request('textDocument/documentHighlight', {
	textDocument: { uri },
	position: { line: callLine, character: arrowCol + 2 },
});
check(
	'documentHighlight returns occurrences',
	Array.isArray(highlight?.result) && highlight.result.length > 0,
	JSON.stringify(highlight),
);

const signature = request('textDocument/signatureHelp', {
	textDocument: { uri },
	position: { line: callLine, character: lines[callLine].indexOf('(') + 1 },
});
check(
	'signatureHelp returns hello(string $name)',
	JSON.stringify(signature?.result?.signatures ?? '').includes('$name'),
	JSON.stringify(signature),
);

const unknown = request('textDocument/nonsense', {});
check(
	'unknown request returns method-not-found',
	unknown?.error?.code === -32601,
	JSON.stringify(unknown),
);

check('notifications produce no response', notify('exit', {}) === null, 'got a reply');

console.log(
	`\nwasm size: ${(bytes.length / (1024 * 1024)).toFixed(1)} MiB raw`,
);

if (failures.length > 0) {
	console.error(`\n${failures.length} check(s) failed`);
	exit(1);
}
console.log('all checks passed');
