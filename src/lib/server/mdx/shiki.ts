import {
	type HighlighterCore,
	createCssVariablesTheme,
	createHighlighterCore,
} from "shiki/core";

import { bundledLanguages } from "shiki/langs";
import d2Lang from "@/../syntaxes/d2.tmLanguage.json";
import pikchrLang from "@/../syntaxes/pikchr.tmLanguage.json";
import { serverLogger } from "../logger";

const shikiInstance: {
	current: HighlighterCore | null;
} = {
	current: null,
};

export const getShiki = async () => {
	const start = performance.now();
	if (shikiInstance.current) {
		serverLogger.debug("shiki instance already created");
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
	serverLogger.debug(`loaded shiki in ${performance.now() - start}`);
	return highlighter;
};
