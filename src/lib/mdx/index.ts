import { compile } from "@mdx-js/mdx";
import {
	transformerNotationDiff,
	transformerNotationErrorLevel,
	transformerNotationFocus,
} from "@shikijs/transformers";

// import rehypeKatex from "rehype-katex";
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
import rehypeMetaString from "./meta";
import { getShiki } from "./shiki";
import remarkToc from "remark-toc";
import rehypeCallouts from "rehype-callouts";
import remarSuperSub from "remark-supersub";
import rehypeTypst from "./typst";

const DEBUG_TREE = false;

const ALLOWED_NODES = ["mdxjsEsm", "mdxJsxFlowElement"];

const makeDebug = (name: string) =>
	DEBUG_TREE ? () => (tree: any) => console.log(name, inspect(tree)) : () => {};

export async function compileMDX(
	content: string,
	urlResolvers: MdxUrlResolvers,
) {
	const result = await compile(content, {
		format: "md",
		outputFormat: "function-body",
		remarkPlugins: [
			[remarkGfm, { singleTilde: false }],
			[remarkFrontmatter, { marker: "-", type: "yaml" }],
			[remarkMdxFrontmatter, { name: "matter" }],
			remarkMath,
			remarkReadingTime,
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
			makeDebug("after metastring"),
			[
				rehypeRaw,
				{
					passThrough: ALLOWED_NODES,
				},
			],
			makeDebug("after raw"),
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
						code: [["className", /^language-./, "math-inline", "math-display"]],
					},
				},
			],
			makeDebug("after sanitize"),
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
			makeDebug("after pretty code"),

			// [rehypeKatex, { throwOnError: false, output: "htmlAndMathml" }],
			rehypeTypst,
			rehypeSlug,
			[
				rehypeTwemoji,
				{
					format: "svg",
					source: "https://cdn.jsdelivr.net/gh/twitter/twemoji@latest",
				} satisfies RehypeTwemojiOptions,
			],
			[rehypeCallouts, { theme: "vitepress" }],
			...getMdxUrl({ resolvers: urlResolvers }),
			makeDebug("after url"),
		],
		remarkRehypeOptions: {
			allowDangerousHtml: true,
			clobberPrefix: "",
		},
	});
	return result.toString();
}
