import { getCompileCache, setCompileCache } from "@uses.ink/cache";
import { IS_DEV, DISABLE_CACHE_DEV, DEFAULT_META } from "@uses.ink/constants";
import { MetaSchema } from "@uses.ink/schemas";
import { logger } from "@uses.ink/server-logger";
import type { MarkdownRawCompileResult, RepoConfig } from "@uses.ink/types";
import { Marked } from "marked";
import parseMatter from "gray-matter";
import { readingTime as rT } from "reading-time-estimator";
import markedShiki from "marked-shiki";
import markedFootnote from "marked-footnote";

import markedTypst from "./typst";
import { getShiki } from "../mdx/shiki";
import DOMPurify from "isomorphic-dompurify";
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
	config?: RepoConfig,
): Promise<MarkdownRawCompileResult> {
	const start = performance.now();
	const cached =
		IS_DEV && DISABLE_CACHE_DEV
			? undefined
			: await getCompileCache(content, "marked");
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

	const marked = await getMarked();
	const html = await marked.parse(matter.content);

	const sanitized = DOMPurify.sanitize(html);

	const result = {
		meta,
		html: sanitized,
		readingTime,
	};

	await setCompileCache(content, result, "marked");

	return result;
}
