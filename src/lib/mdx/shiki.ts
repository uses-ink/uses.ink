import {
	createHighlighterCore,
	createCssVariablesTheme,
	type HighlighterCore,
} from "shiki/core";

import { bundledLanguages } from "shiki/langs";

const shikiInstance: {
	current: HighlighterCore | null;
} = {
	current: null,
};

export const getShiki = async () => {
	if (shikiInstance.current) {
		return shikiInstance.current;
	}

	const theme = createCssVariablesTheme({
		name: "default",
		variablePrefix: "--shiki-",
		variableDefaults: {},
		fontStyle: true,
	});

	const highlighter = await createHighlighterCore({
		themes: [theme],
		langs: Object.values(bundledLanguages),
		loadWasm: import("shiki/wasm"),
	});

	shikiInstance.current = highlighter;

	return highlighter;
};
