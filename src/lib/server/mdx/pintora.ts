import type { Plugin } from "unified";
import type { Root } from "hast";
import { rehypeCodeHook, type MapLike } from "@beoe/rehype-code-hook";
import { getConfig, render, setConfig, type PintoraConfig } from "@pintora/cli";
// import type { CLIRenderOptions } from '@pintora/cli/lib/render'
// import type { PintoraConfig } from '@pintora/core/lib/config'

type DeepPartial<T> = {
	[P in keyof T]?: DeepPartial<T[P]>;
};

const theme = {
	primaryColor: "#fff",
	secondaryColor: "var(--secondary)",
	teritaryColor: "var(--teritary)",

	primaryLineColor: "var(--primary-line)",
	secondaryLineColor: "var(--secondary-line)",

	textColor: "var(--foreground)",
	primaryTextColor: "var(--primary-foreground)",
	secondaryTextColor: "var(--secondary-foreground)",
	teritaryTextColor: "var(--secondary-foreground)",

	primaryBorderColor: "var(--border)",
	secondaryBorderColor: "var(--border)",

	canvasBackground: "var(--background)",
	background1: "var(--background)",
	lightestBackground: "var(--background)",
	groupBackground: "var(--background)",

	noteBackground: "var(--card-background)",
	noteTextColor: "var(--card-foreground)",
} satisfies DeepPartial<PintoraConfig["themeConfig"]["themeVariables"]>;
const pintoraSvg = async (code: string, className?: string) => {
	setConfig({
		themeConfig: {
			themeVariables: theme,
		},
	});
	const svg = (await render({
		code,
		mimeType: "image/svg+xml",
		pintoraConfig: getConfig(),
		backgroundColor: "var(--background)",
	})) as string;

	// can't remove width and height, because there is no viewport
	// svg = svg.replace(/\s+width="\d+[^"]+"/, "");
	// svg = svg.replace(/\s+height="\d+[^"]+"/, "");
	return `<figure class="pintora ${className || ""}">${svg}</figure>`;
};

export type RehypePintoraConfig = {
	cache?: MapLike;
	class?: string;
};

export const rehypePintora: Plugin<[RehypePintoraConfig?], Root> = (
	options = {},
) => {
	const salt = { class: options.class };
	// @ts-expect-error
	return rehypeCodeHook({
		...options,
		salt,
		language: "pintora",
		code: ({ code }) => pintoraSvg(code, options.class),
	});
};

export default rehypePintora;
