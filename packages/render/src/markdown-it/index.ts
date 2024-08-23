import { getCompileCache, setCompileCache } from "@uses.ink/cache";
import { DEFAULT_META, DISABLE_CACHE_DEV, IS_DEV } from "@uses.ink/constants";
import { MetaSchema } from "@uses.ink/schemas";
import { logger } from "@uses.ink/server-logger";
import type { MarkdownRawCompileResult, RepoConfig } from "@uses.ink/types";
import parseMatter from "gray-matter";
import { readingTime as rT } from "reading-time-estimator";
import MarkdownIt from "markdown-it";
import DOMPurify from "isomorphic-dompurify";
import { fromHighlighter } from "@shikijs/markdown-it/core";
import {
	bundledLanguages,
	createCssVariablesTheme,
	getSingletonHighlighter,
} from "shiki";
import d2Lang from "@uses.ink/syntaxes/d2.tmLanguage.json";
import pikchrLang from "@uses.ink/syntaxes/pikchr.tmLanguage.json";

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
	config?: RepoConfig,
): Promise<MarkdownRawCompileResult> {
	const start = performance.now();
	const cached =
		IS_DEV && DISABLE_CACHE_DEV
			? undefined
			: await getCompileCache(content, "markdownIt");
	if (cached) {
		logger.debug(`Cache hit in ${performance.now() - start}ms`);
		return cached;
	}
	const matter = parseMatter(content);

	let meta = MetaSchema.parse(matter.data);

	meta = {
		...DEFAULT_META,
		...config,
		...meta,
	};

	const readingTime = meta.readingTime ? rT(matter.content) : undefined;

	logger.debug("meta", meta);
	logger.debug("Cache miss");

	const markdownit = await getMarkdownIt();
	const html = markdownit.render(matter.content);

	const sanitized = DOMPurify.sanitize(html);

	const result = {
		meta,
		html: sanitized,
		readingTime,
	};

	await setCompileCache(content, result, "markdownIt");

	return result;
}
