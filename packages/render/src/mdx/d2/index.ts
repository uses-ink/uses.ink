import { rehypeCodeHook, type MapLike } from "@beoe/rehype-code-hook";
import type { Root } from "hast";
import type { Plugin } from "unified";
import { loadWasm } from "./wasm";
import { generateD2Diagram } from "./cli";
import { getAttributes } from "./attributes";
import { getD2Cache, setD2Cache } from "@uses.ink/cache";
import { logger } from "@uses.ink/server-logger";
import type {
	D2Config,
	D2DiagramAttributes,
	D2Size,
	D2UserConfig,
} from "@uses.ink/types";
import { D2ConfigSchema } from "@uses.ink/schemas";

export type RenderOptions = {
	dsl: string;
	dark: boolean;
	sketch: boolean;
	center?: boolean;
	padding?: number;
	scale?: number;
};

export type RenderResult = {
	svg: string;
};

type Meta = Record<string, string | boolean | number>;

const darkifySVG = (svg: string) => {
	return svg
		.replace(/class="(.+?)"/g, (match, classes: string) => {
			return `class="${classes
				.split(" ")
				.filter((c) => !!c.trim())
				.map((c) => `dark-${c}`)
				.join(" ")}"`;
		})
		.replace(/<style type="text\/css">(?:.|\n)+<\/style>/, (css) => {
			return css.replace(/\.(.+? ?) ?{/g, (_, classes: string) => {
				return `.${classes
					.split(" .")
					.filter((c) => !!c.trim())
					.map((c, i) => `${i > 0 ? "." : ""}dark-${c}`)
					.join(" ")}{`;
			});
		});
};

const removeBackground = (svg: string) => {
	return svg.replace(/<rect.*class=".*fill-N7".*\/>/, "");
};

const d2SvgWasm = async (code: string, meta: Meta, className?: string) => {
	const start = Date.now();

	const { render } = await loadWasm();

	logger.debug("WASM loaded in", Date.now() - start, "ms");

	// disable output from the wasm module
	// @ts-expect-error
	console._log = console.log;
	console.log = () => {};
	const startRender = Date.now();
	let { svg: lightSvg } = render({
		dsl: code,
		dark: false,
		sketch: !!(meta.sketch ?? false),
	});

	const render1 = Date.now();
	let { svg: darkSvg } = render({
		dsl: code,
		dark: true,
		sketch: !!(meta.sketch ?? false),
	});
	const render2 = Date.now();
	// re-enable console.log
	// @ts-expect-error
	console.log = console._log;
	console.log(
		"Rendered in avg.",
		(render1 - startRender + (render2 - render1)) / 2,
		"ms",
	);

	lightSvg = removeBackground(lightSvg);

	darkSvg = removeBackground(darkifySVG(darkSvg));

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

export type RehypeD2WasmConfig = {
	cache?: MapLike;
	class?: string;
};

export const rehypeD2Wasm: Plugin<[RehypeD2WasmConfig?], Root> = (
	options = {},
) => {
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
			return d2SvgWasm(code, parsedMeta ?? {}, options.class);
		},
	});
};

const renderSVG = async (
	config: D2Config,
	attributes: D2DiagramAttributes,
	code: string,
	dark: boolean,
) => {
	const start = performance.now();
	let dsl = code;
	const darkConfig = `
vars: {
  d2-config: {
    theme-overrides: {
      B1: "#F0F0F0"
      B2: "#989898"
      B3: "#707070"
      B4: "#303030"
      B5: "#212121"
      B6: "#111111"
      
      AA2: "#989898"
      AA4: "#303030"
      AA5: "#212121"
      
      AB4: "#303030"

    }
  }
}`;
	if (dark) {
		dsl = `${darkConfig}\n\n${code}`;
	}
	const cached = await getD2Cache({ attributes, code: dsl, config });
	if (cached) {
		logger.debug(`d2 cache hit in ${performance.now() - start}ms`);
		return cached;
	}
	let { svg, size } = await generateD2Diagram(config, attributes, dsl, dark);
	logger.debug(`d2 render in ${performance.now() - start}ms`);

	svg = removeBackground(svg);
	if (dark) {
		svg = darkifySVG(svg);
	}

	const res = {
		svg: `<div class="d2-${dark ? "dark" : "light"}" style="width:100%;">${svg}</div>`,
		size,
	};
	setD2Cache({ attributes, code: dsl, config }, res);

	return res;
};

export const rehypeD2CLI: Plugin<[D2UserConfig?], Root> = (options = {}) => {
	const config = D2ConfigSchema.parse(options);

	// @ts-expect-error
	return rehypeCodeHook({
		...options,
		language: "d2",
		code: async ({ code, meta }) => {
			const start = performance.now();
			const attributes = getAttributes(meta);
			logger.debug(`parsed attributes in ${performance.now() - start}ms`);
			if (attributes.doNotRender) {
				return undefined;
			}

			const { svg: darkSVG } = await renderSVG(config, attributes, code, true);
			const { svg: lightSVG } = await renderSVG(
				config,
				attributes,
				code,
				false,
			);
			logger.debug(
				`d2 rendered both light+dark in ${performance.now() - start}ms`,
			);

			return `<figure class="d2 not-prose ${attributes.sketch ?? config.sketch ? "sketch" : ""}">${darkSVG}${lightSVG}<figcaption>${attributes.title ?? ""}</figcaption></figure>`;
		},
	});
};

export function computeImgSize(
	htmlAttributes: Record<string, string>,
	attributes: D2DiagramAttributes,
	size: D2Size,
) {
	if (attributes.width !== undefined) {
		htmlAttributes.width = String(attributes.width);

		if (size) {
			const aspectRatio = size.height / size.width;
			htmlAttributes.height = String(
				Math.round(attributes.width * aspectRatio),
			);
		}
	} else if (size) {
		htmlAttributes.width = String(size.width);
		htmlAttributes.height = String(size.height);
	} else {
		htmlAttributes.width = "100%";
	}
}
