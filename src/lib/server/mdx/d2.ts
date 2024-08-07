import type { Plugin } from "unified";
import type { Root } from "hast";
import { rehypeCodeHook, type MapLike } from "@beoe/rehype-code-hook";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { brotliDecompressSync } from "node:zlib";

import "../wasm_exec";

const current = {
	instanciated: false,
};

const loadWasm = async () => {
	if (current.instanciated) {
		console.log("WASM already instanciated");
		return;
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
};

type Meta = Record<string, string | boolean | number>;

const d2Svg = async (code: string, meta: Meta, className?: string) => {
	await loadWasm();

	// @ts-expect-error
	const { d2RenderSVG } = globalThis;
	// disable output from the wasm module
	// @ts-expect-error
	console._log = console.log;
	console.log = () => {};
	const { svg: light } = JSON.parse(
		d2RenderSVG({ dsl: code, dark: false, sketch: meta.sketch ?? false }),
	) as {
		svg: string;
	};
	const { svg: dark } = JSON.parse(
		d2RenderSVG({ dsl: code, dark: true, sketch: meta.sketch ?? false }),
	) as {
		svg: string;
	};
	// re-enable console.log
	// @ts-expect-error
	console.log = console._log;

	// remove the background color from both the dard and light svgs
	// /<rect .* class=".*(fill-N7)".*<\/rect>/
	const lightSvg = light.replace(/<rect.*class=".*fill-N7".*\/>/, "");
	let darkSvg = dark.replace(/<rect.*class=".*fill-N7".*\/>/, "");

	// Replace all the classes in the dark svg with dark-* prefix
	darkSvg = darkSvg.replace(/class="(.+?)"/g, (match, classes: string) => {
		return `class="${classes
			.split(" ")
			.filter((c) => !!c.trim())
			.map((c) => `dark-${c}`)
			.join(" ")}"`;
	});

	// Replace the classes in embedded css
	darkSvg = darkSvg.replace(
		/<style type="text\/css">(?:.|\n)+<\/style>/,
		(css) => {
			return css.replace(/\.(.+? ?) ?{/g, (_, classes: string) => {
				return `.${classes
					.split(" .")
					.filter((c) => !!c.trim())
					.map((c, i) => `${i > 0 ? "." : ""}dark-${c}`)
					.join(" ")}{`;
			});
		},
	);
	//// IMGS
	// return `<div class="d2 ${className ?? ""}">
	//     <img class="d2-dark"${meta.title ? `alt="${meta.title}"` : ""} src="data:image/svg+xml,${encodeURIComponent(darkSvg)}"></img>
	//     <img class="d2-light"${meta.title ? `alt="${meta.title}"` : ""} src="data:image/svg+xml,${encodeURIComponent(lightSvg)}"></img>
	// </div>`;
	//// DIVS
	// return `<div class="d2 ${className ?? ""}">
	//     <div class="d2-dark">${darkSvg}</div>
	//     <div class="d2-light">${lightSvg}</div>
	// </div>`;
	//// FIGURES
	return `<figure class="d2 not-prose ${className ?? ""}">
        <div class="d2-dark">${darkSvg}</div>
        <div class="d2-light">${lightSvg}</div>
        <figcaption>${meta.title ?? ""}</figcaption>
    </figure>`;
};

export type RehypeD2Config = {
	cache?: MapLike;
	class?: string;
};

export const rehypeD2: Plugin<[RehypeD2Config?], Root> = (options = {}) => {
	const salt = { class: options.class };
	// @ts-expect-error
	return rehypeCodeHook({
		...options,
		salt,
		language: "d2",
		code: ({ code, meta }) => {
			// ```d2 bool_var string_var="value" int_var=42

			// Parse the meta, do not split with " " because the string value can contain spaces
			const parsedMeta = meta
				?.split(/ (?=(?:[^"]|"[^"]*")*$)/)
				.map((s) => s.split("="))
				.reduce((acc, [key, value]) => {
					if (value === undefined) {
						// biome-ignore lint/performance/noAccumulatingSpread: <explanation>
						return { ...acc, [key]: true };
					}
					// biome-ignore lint/performance/noAccumulatingSpread: <explanation>
					return { ...acc, [key]: value.replace(/"/g, "") };
				}, {}) as Meta;

			// console.log("parsedMeta", parsedMeta);
			if (parsedMeta?.doNotRender || parsedMeta?.doNotRender === "true") {
				return undefined;
			}
			return d2Svg(code, parsedMeta ?? {}, options.class);
		},
	});
};

export default rehypeD2;
