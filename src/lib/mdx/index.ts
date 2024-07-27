import { compile, runSync } from "@mdx-js/mdx";
import {
	transformerNotationDiff,
	transformerNotationErrorLevel,
	transformerNotationFocus,
} from "@shikijs/transformers";
// @ts-ignore
import * as runtime from "react/jsx-runtime";
import type { readingTime as getReadingTime } from "reading-time-estimator";
import rehypeKatex from "rehype-katex";
import rehypeRaw from "rehype-raw";
import rehypeSanitize, { defaultSchema } from "rehype-sanitize";
import rehypeSlug from "rehype-slug";
import remarkFrontmatter from "remark-frontmatter";
import remarkGfm from "remark-gfm";
import remarkMath from "remark-math";
import remarkMdxFrontmatter from "remark-mdx-frontmatter";
import { inspect } from "unist-util-inspect";
import { remarkReadingTime } from "./read-time";
import { type MdxUrlResolvers, getMdxUrl } from "./url";
import { rehypeTwemoji, type RehypeTwemojiOptions } from "rehype-twemoji";
import remarkEmoji from "remark-emoji";
import rehypePrettyCode from "rehype-pretty-code";
import { MetaSchema } from "../types";
import rehypeMetaString from "./meta";
import { getShiki } from "./shiki";
import remarkToc from "remark-toc";
import remarkCallout from "@r4ai/remark-callout";

const DEBUG_TREE = false;

const ALLOWED_NODES = ["mdxjsEsm", "mdxJsxFlowElement"];

export async function compileMDX(
	content: string,
	urlResolvers: MdxUrlResolvers,
) {
	const result = await compile(content, {
		format: "md",
		outputFormat: "function-body",
		remarkPlugins: [
			remarkGfm,
			[remarkFrontmatter, { marker: "-", type: "yaml" }],
			[remarkMdxFrontmatter, { name: "matter" }],
			remarkMath,
			remarkReadingTime,
			[remarkToc, { maxDepth: 3 }],
			remarkCallout,
			remarkEmoji,
		],
		rehypePlugins: [
			DEBUG_TREE
				? () => (tree) => console.log("before", inspect(tree), "\n\n")
				: [() => {}],
			// Moves `data.meta` to `properties.metastring` for the `code` element node
			// Because `rehype-raw` strips `data` from all nodes, which may contain useful information.
			rehypeMetaString,
			DEBUG_TREE
				? () => (tree) => console.log("after meta", inspect(tree), "\n\n")
				: [() => {}],
			[
				rehypeRaw,
				{
					passThrough: ALLOWED_NODES,
				},
			],
			DEBUG_TREE
				? () => (tree) => console.log("after raw", inspect(tree), "\n\n")
				: [() => {}],
			[
				rehypeSanitize,
				{
					...defaultSchema,
					// @ts-ignore
					unknownNodeHandler: (state, node) => {
						if (DEBUG_TREE) {
							console.log("Unknown node", node);
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
					},
				},
			],
			DEBUG_TREE
				? () => (tree) => console.log("after sanitize", inspect(tree), "\n\n")
				: [() => {}],
			[
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
			DEBUG_TREE
				? () => (tree) =>
						console.log("after pretty-code", inspect(tree), "\n\n")
				: [() => {}],

			[rehypeKatex, { output: "mathml" }],
			rehypeSlug,
			[
				rehypeTwemoji,
				{
					format: "svg",
					source: "https://cdn.jsdelivr.net/gh/twitter/twemoji@latest",
				} satisfies RehypeTwemojiOptions,
			],
			...getMdxUrl({ resolvers: urlResolvers }),
			DEBUG_TREE
				? () => (tree) => console.log("end", inspect(tree), "\n\n")
				: [() => {}],
		],
		remarkRehypeOptions: {
			allowDangerousHtml: true,
		},
	});
	return result.toString();
}

export function runMDX(code: string) {
	// Need to run sync so the server build also has full html
	const mdx = runSync(code, runtime as any);

	const { default: Content, matter, readingTime } = mdx;
	console.log("runMDX -> readingTime", readingTime);
	console.log("runMDX -> matter", matter);
	const meta = MetaSchema.safeParse(matter ?? {});
	console.log("runMDX -> meta", meta);

	return {
		Content,
		meta,
		readingTime: (readingTime as ReturnType<typeof getReadingTime>) ?? {},
	};
}
