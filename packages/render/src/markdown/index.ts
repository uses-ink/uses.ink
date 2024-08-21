import { compile } from "@mdx-js/mdx";

import {
	transformerNotationDiff,
	transformerNotationErrorLevel,
	transformerNotationFocus,
} from "@shikijs/transformers";
import { logger } from "@uses.ink/server-logger";
// import rehypeKatex from "rehype-katex";
import type { MarkdownCompileResult, RepoConfig, Meta } from "@uses.ink/types";
import { MetaSchema } from "@uses.ink/schemas";
import parseMatter from "gray-matter";
import rehypeCallouts from "rehype-callouts";
import rehypeKatex from "rehype-katex";
import rehypePrettyCode from "rehype-pretty-code";
import rehypeRaw from "rehype-raw";
import rehypeSanitize, { defaultSchema } from "rehype-sanitize";
import rehypeSlug from "rehype-slug";
import { type RehypeTwemojiOptions, rehypeTwemoji } from "rehype-twemoji";
import remarkEmoji from "remark-emoji";
import remarkGfm from "remark-gfm";
import remarkMath from "remark-math";
import remarSuperSub from "remark-supersub";
import remarkToc from "remark-toc";
import { match } from "ts-pattern";
import { inspect } from "unist-util-inspect";
import rehypeMetaString from "./meta";
import { remarkReadingTime } from "./read-time";
import { getShiki } from "./shiki";
import rehypeTypst from "./typst";
import { type MdxUrlResolvers, getMdxUrl } from "./url";

import { getCompileCache, setCompileCache } from "@uses.ink/cache";
import { rehypeD2CLI } from "./d2";
import { rehypePikchr } from "./pikchr";

import {
	ALLOWED_NODES,
	DEBUG_TREE,
	DEFAULT_META,
	DISABLE_CACHE_DEV,
	IS_DEV,
} from "@uses.ink/constants";
import rehypeCopy from "./copy";
import { readingTime as rT } from "reading-time-estimator";

export async function compileMarkdown(
	content: string,
	urlResolvers: MdxUrlResolvers,
	config?: RepoConfig,
): Promise<MarkdownCompileResult> {
	const start = performance.now();
	const cached =
		IS_DEV && DISABLE_CACHE_DEV ? undefined : await getCompileCache(content);
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

	const readingTime = meta.readingTime ? rT(content) : undefined;

	logger.debug("meta", meta);
	logger.debug("Cache miss");

	let lastTimestamp = performance.now();
	const makeDebug = (name: string) =>
		DEBUG_TREE
			? () => (tree: unknown) => logger.debug(inspect(tree))
			: IS_DEV
				? () => (tree: unknown) => {
						const now = performance.now();
						logger.debug(name, now - lastTimestamp, "ms");
						lastTimestamp = now;
					}
				: () => () => {};
	const result = await compile(matter.content, {
		format: "md",
		outputFormat: "function-body",
		remarkPlugins: [
			[remarkGfm, { singleTilde: false }],
			remarkMath,
			...(meta.readingTime ? [remarkReadingTime] : []),
			[
				remarkToc,
				{
					maxDepth: 3,
					skip: ".*<!--( ?)no-?toc( ?)-->",
					heading: ".*<!--( ?)toc( ?)-->",
				},
			],
			remarkEmoji,
			remarSuperSub,
		],
		rehypePlugins: [
			makeDebug("start"),
			// Moves `data.meta` to `properties.metastring` for the `code` element node
			// Because `rehype-raw` strips `data` from all nodes, which may contain useful information.
			rehypeMetaString,
			makeDebug("metastring"),
			[
				rehypeRaw,
				{
					passThrough: ALLOWED_NODES,
				},
			],
			makeDebug("raw"),
			[
				rehypeSanitize,
				{
					...defaultSchema,
					// @ts-ignore
					unknownNodeHandler: (state, node) => {
						if (DEBUG_TREE) {
							logger.debug("Unknown node", node);
						}
						if (ALLOWED_NODES.includes(node.type)) {
							return node;
						}
					},
					attributes: {
						...defaultSchema.attributes,
						"*": [
							...(defaultSchema.attributes?.["*"] ?? []),
							"className",
							"style",
							"metastring",
							"dataCallout",
							"dataCalloutType",
							"dataCalloutTitle",
							"dataCalloutBody",
						],
						code: [["className", /^language-./, "math-inline", "math-display"]],
					},
				},
			],
			makeDebug("sanitize"),
			// [rehypePintora, { class: "pintora" }],
			// [rehypePenrose, { class: "penrose" }],
			rehypeD2CLI,
			makeDebug("d2cli"),
			rehypePikchr,
			makeDebug("pikchr"),
			// rehypeD2Wasm,

			[rehypeCopy, { defaultCopy: meta.defaultCopy }],
			meta.noHighlight
				? []
				: [
						rehypePrettyCode,
						{
							getHighlighter: getShiki,
							theme: "default",
							transformers: [
								transformerNotationDiff(),
								transformerNotationFocus(),
								transformerNotationErrorLevel(),
							],
						},
					],
			makeDebug("pretty code"),
			match(meta.mathEngine)
				.with("katex", () => [
					rehypeKatex,
					{ throwOnError: false, output: "html" },
				])
				// Use typst as default
				.otherwise(() => [rehypeTypst, undefined]) as any,
			makeDebug("math"),
			rehypeSlug,
			makeDebug("slug"),
			[
				rehypeTwemoji,
				{
					format: "svg",
					source: "https://cdn.jsdelivr.net/gh/twitter/twemoji@latest",
				} satisfies RehypeTwemojiOptions,
			],
			makeDebug("twemoji"),
			[rehypeCallouts, { theme: "vitepress" }],
			makeDebug("callouts"),
			...getMdxUrl({ resolvers: urlResolvers }),
			makeDebug("url"),
		],
		remarkRehypeOptions: {
			allowDangerousHtml: true,
			clobberPrefix: "",
		},
	});
	logger.debug(`Compiled in ${performance.now() - start}ms`);
	const compiled = { meta: meta, runnable: result.toString(), readingTime };
	await setCompileCache(content, compiled);
	return compiled;
}

export * from "./d2";
