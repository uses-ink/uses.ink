import { getCompileCache, setCompileCache } from "@uses.ink/cache";
import { DISABLE_CACHE_DEV, IS_DEV } from "@uses.ink/constants";
import { logger } from "@uses.ink/server-logger";
import type { Meta } from "@uses.ink/types";
import { Marked } from "marked";
import markedFootnote from "marked-footnote";
import markedShiki from "marked-shiki";

import DOMPurify from "isomorphic-dompurify";
import { getShiki } from "../mdx/shiki";
import markedTypst from "./typst";
// @ts-ignore
import preTemplate from "@uses.ink/components/templates/pre.html?raw";
import markedTwemoji from "./twemoji";

const instance: { current: Marked | null } = { current: null };

const MARKED_PLUGINS = [
	markedTwemoji,
	markedShiki({
		container: preTemplate,
		highlight: async (code, lang, props) => {
			const shiki = await getShiki();
			return shiki.codeToHtml(code, { theme: "default", lang });
		},
	}),
	markedFootnote(),
	markedTypst({ nonStandard: true }),
];

export async function getMarked() {
	if (instance.current) {
		return instance.current;
	}

	instance.current = new Marked(
		{
			gfm: true,
			breaks: true,
		},
		...MARKED_PLUGINS,
	);
	return instance.current;
}

export async function compileMarkdownMarked(
	content: string,
	meta?: Meta,
): Promise<string> {
	const start = performance.now();
	const cached =
		IS_DEV && DISABLE_CACHE_DEV
			? undefined
			: await getCompileCache(content, "marked");
	if (cached) {
		logger.debug(`Cache hit in ${performance.now() - start}ms`);
		return cached;
	}

	logger.debug("meta", meta);
	logger.debug("Cache miss");

	const marked = await getMarked();
	const html = await marked.parse(content);

	const sanitized = DOMPurify.sanitize(html);

	await setCompileCache(content, sanitized, "marked");

	return sanitized;
}
