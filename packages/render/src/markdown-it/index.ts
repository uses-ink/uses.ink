import { fromHighlighter } from "@shikijs/markdown-it/core";
import { getCompileCache, setCompileCache } from "@uses.ink/cache";
import { DISABLE_CACHE_DEV, IS_DEV } from "@uses.ink/constants";
import { logger } from "@uses.ink/server-logger";
import d2Lang from "@uses.ink/syntaxes/d2.tmLanguage.json";
import pikchrLang from "@uses.ink/syntaxes/pikchr.tmLanguage.json";
import type { Meta } from "@uses.ink/types";
import DOMPurify from "isomorphic-dompurify";
import MarkdownIt from "markdown-it";
import {
	bundledLanguages,
	createCssVariablesTheme,
	getSingletonHighlighter,
} from "shiki";

const instance: { current: MarkdownIt | null } = { current: null };

export async function getMarkdownIt() {
	if (instance.current) {
		return instance.current;
	}

	const md = new MarkdownIt({
		html: true,
		linkify: true,
		typographer: true,
	});
	md.use(
		fromHighlighter(
			await getSingletonHighlighter({
				themes: [
					createCssVariablesTheme({
						name: "default",
						variablePrefix: "--shiki-",
						variableDefaults: {},
						fontStyle: true,
					}),
				],
				langs: [...Object.values(bundledLanguages), pikchrLang, d2Lang] as any,
			}),
			{ theme: "default" },
		),
	);
	instance.current = md;
	return instance.current;
}

export async function compileMarkdownIt(
	content: string,
	meta: Meta,
): Promise<string> {
	const markdownit = await getMarkdownIt();
	const html = markdownit.render(content);

	const sanitized = DOMPurify.sanitize(html);

	return sanitized;
}
