import { readFileSync } from "node:fs";
import { join } from "node:path";
import { brotliDecompressSync } from "node:zlib";
import type { RenderOptions, RenderResult } from ".";
import "../../wasm_exec";

const current = {
	instanciated: false,
};

export const loadWasm = async () => {
	if (current.instanciated) {
		console.log("WASM already instanciated");
		return {
			// @ts-expect-error
			render: globalThis.d2RenderSVG as (
				options: RenderOptions,
			) => RenderResult,
		};
	}
	const wasmPath = join(process.cwd(), "public", "wasm", "d2.wasm.br");
	const wasmCompressedBuffer = readFileSync(wasmPath);
	console.log(
		"wasmCompressedBuffer",
		`${(wasmCompressedBuffer.length / 1024 / 1024).toFixed(1)}MB`,
	);
	const wasmBuffer = brotliDecompressSync(wasmCompressedBuffer);
	console.log(
		"wasmBuffer",
		`${(wasmBuffer.length / 1024 / 1024).toFixed(1)}MB`,
	);
	// @ts-expect-error
	const go = new Go();

	const instance = new WebAssembly.Instance(
		new WebAssembly.Module(wasmBuffer),
		go.importObject,
	);

	go.run(instance);
	console.log("WASM instanciated");
	current.instanciated = true;
	return {
		render: (options: RenderOptions) => {
			// @ts-expect-error
			return JSON.parse(globalThis.d2RenderSVG(options)) as RenderResult;
		},
	};
};
