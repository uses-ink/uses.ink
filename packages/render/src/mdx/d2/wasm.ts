import "../../../wasm/wasm_exec.js";

import { readFileSync } from "node:fs";
import { join } from "node:path";
import { brotliDecompressSync } from "node:zlib";
import type { RenderOptions, RenderResult } from "./index.js";
import { logger } from "@uses.ink/server-logger";

const current = {
	instanciated: false,
};

export const loadWasm = async () => {
	if (current.instanciated) {
		logger.debug("WASM already instanciated");
		return {
			render: globalThis.d2RenderSVG as (
				options: RenderOptions,
			) => RenderResult,
		};
	}
	const wasmPath = join(process.cwd(), "wasm", "d2.wasm.br");
	const wasmCompressedBuffer = readFileSync(wasmPath);
	logger.debug(
		"wasmCompressedBuffer",
		`${(wasmCompressedBuffer.length / 1024 / 1024).toFixed(1)}MB`,
	);
	const wasmBuffer = brotliDecompressSync(wasmCompressedBuffer);
	logger.debug(
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
	logger.debug("WASM instanciated");
	current.instanciated = true;
	return {
		render: (options: RenderOptions) => {
			// @ts-expect-error
			return JSON.parse(globalThis.d2RenderSVG(options)) as RenderResult;
		},
	};
};
