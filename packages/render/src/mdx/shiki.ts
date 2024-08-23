import { createCssVariablesTheme } from "shiki/core";

import d2Lang from "@uses.ink/syntaxes/d2.tmLanguage.json";
import pikchrLang from "@uses.ink/syntaxes/pikchr.tmLanguage.json";
import { getSingletonHighlighter } from "shiki";
import { bundledLanguages } from "shiki/langs";

export const getShiki = async () => {
	const highlighter = await getSingletonHighlighter({
		themes: [
			createCssVariablesTheme({
				name: "default",
				variablePrefix: "--shiki-",
				variableDefaults: {},
				fontStyle: true,
			}),
		],
		langs: [...Object.values(bundledLanguages), pikchrLang, d2Lang] as any,
	});

	return highlighter;
};
