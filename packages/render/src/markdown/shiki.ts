import {
	type HighlighterCore,
	createCssVariablesTheme,
	createHighlighterCore,
} from "shiki/core";

import { bundledLanguages } from "shiki/langs";
import d2Lang from "@uses.ink/syntaxes/d2.tmLanguage.json";
import pikchrLang from "@uses.ink/syntaxes/pikchr.tmLanguage.json";
import { logger } from "@uses.ink/server-logger";

const shikiInstance: {
	current: HighlighterCore | null;
} = {
	current: null,
};

export const getShiki = async () => {
	const start = performance.now();
	if (shikiInstance.current) {
		logger.debug("shiki instance already created");
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
		langs: [...Object.values(bundledLanguages), pikchrLang, d2Lang] as any,
		loadWasm: import("shiki/wasm"),
	});

	shikiInstance.current = highlighter;
	logger.debug(`loaded shiki in ${performance.now() - start}`);
	return highlighter;
};
