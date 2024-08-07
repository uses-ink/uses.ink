import type { Plugin } from "unified";
import type { Root } from "hast";
import { rehypeCodeHook, type MapLike } from "@beoe/rehype-code-hook";
import "global-jsdom/register";
import {
	compile,
	optimize,
	showError,
	toInteractiveSVG,
	toSVG,
} from "@penrose/core";

const DELIMITER = "\n---\n";

const penroseSvg = async (code: string, className?: string) => {
	// const [substance, style, domain, variation] = code.split("\n---\n");
	const parts = code.split(DELIMITER);
	const substance = parts[0];
	const style = parts[1];
	const domain = parts[2];
	const variation = parts[3];
	const compiled = await compile({ substance, style, domain, variation });
	if (compiled.isErr()) return console.error(showError(compiled.error));
	const optimized = optimize(compiled.value);
	if (optimized.isErr()) return console.error(showError(optimized.error));
	const svg = await toSVG(optimized.value, async () => undefined, "default");
	return svg.outerHTML;
};

export type RehypePenroseConfig = {
	cache?: MapLike;
	class?: string;
};

export const rehypePenrose: Plugin<[RehypePenroseConfig?], Root> = (
	options = {},
) => {
	const salt = { class: options.class };
	// @ts-expect-error
	return rehypeCodeHook({
		...options,
		salt,
		language: "penrose",
		code: ({ code }) => penroseSvg(code, options.class),
	});
};

export default rehypePenrose;
