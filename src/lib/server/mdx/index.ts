import { compile } from "@mdx-js/mdx";

import {
	transformerNotationDiff,
	transformerNotationErrorLevel,
	transformerNotationFocus,
} from "@shikijs/transformers";

// import rehypeKatex from "rehype-katex";
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
import type { z, ZodError } from "zod";
import { type ConfigSchema, MetaSchema } from "../../types";
import { serverLogger } from "../logger";
import rehypeMetaString from "./meta";
import { remarkReadingTime } from "./read-time";
import { getShiki } from "./shiki";
import rehypeTypst from "./typst";
import { type MdxUrlResolvers, getMdxUrl } from "./url";

import { getCompileCache, setCompileCache } from "../cache";
import { rehypeD2CLI } from "./d2";
import { rehypePikchr } from "./pikchr";

import { IS_DEV } from "@/lib/constants";

const DEBUG_TREE = false;
const DISABLE_CACHE = true;

const ALLOWED_NODES = ["mdxjsEsm", "mdxJsxFlowElement"];

export type CompileResult = {
	meta: z.infer<typeof MetaSchema>;
	runnable: string;
};

export async function compileMDX(
	content: string,
	urlResolvers: MdxUrlResolvers,
	config?: z.infer<typeof ConfigSchema>,
): Promise<CompileResult | ZodError<z.output<typeof MetaSchema>>> {
	const start = performance.now();
	const cached =
		IS_DEV && DISABLE_CACHE ? undefined : await getCompileCache(content);
	if (cached) {
		serverLogger.debug(`Cache hit in ${performance.now() - start}ms`);
		return cached;
	}
	const matter = parseMatter(content);

	const meta = MetaSchema.safeParse(matter.data);

	if (!meta.success) {
		return meta.error;
	}

	meta.data = {
		...config,
		...meta.data,
	};
	serverLogger.debug("Cache miss");

	let lastTimestamp = performance.now();
	const makeDebug = (name: string) =>
		DEBUG_TREE
			? () => (tree: any) => serverLogger.debug(inspect(tree))
			: IS_DEV
				? () => (tree: any) => {
						const now = performance.now();
						serverLogger.debug(name, now - lastTimestamp, "ms");
						lastTimestamp = now;
					}
				: () => () => {};
	const result = await compile(matter.content, {
		format: "md",
		outputFormat: "function-body",
		remarkPlugins: [
			[remarkGfm, { singleTilde: false }],
			remarkMath,
			...(meta.data.readingTime ? [remarkReadingTime] : []),
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
							serverLogger.debug("Unknown node", node);
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
			meta.data.noHighlight
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
			match(meta.data.mathEngine)
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
	serverLogger.debug(`Compiled in ${performance.now() - start}ms`);
	const compiled = { meta: meta.data, runnable: result.toString() };
	await setCompileCache(content, compiled);
	return compiled;
}
