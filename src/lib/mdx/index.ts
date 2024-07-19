import { compile, runSync } from "@mdx-js/mdx";
import remarkGfm from "remark-gfm";
import remarkFrontmatter from "remark-frontmatter";
import remarkMdxFrontmatter from "remark-mdx-frontmatter";
import remarkMath from "remark-math";
// @ts-ignore
import * as runtime from "react/jsx-runtime";
import { getMdxUrl, type MdxUrlResolvers } from "./url";
import rehypeShiki from "@shikijs/rehype";
import rehypeKatex from "rehype-katex";
import {
	transformerNotationDiff,
	transformerNotationHighlight,
	transformerNotationWordHighlight,
	transformerNotationFocus,
	transformerNotationErrorLevel,
	transformerMetaHighlight,
} from "@shikijs/transformers";

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
			remarkMdxFrontmatter,
			remarkMath,
		],
		rehypePlugins: [
			...getMdxUrl({ resolvers: urlResolvers }),
			[rehypeKatex, { output: "mathml" }],
			[
				rehypeShiki,
				{
					themes: {
						dark: "github-dark",
						light: "github-light",
					},
					transformers: [
						transformerNotationDiff(),
						transformerNotationHighlight(),
						transformerNotationWordHighlight(),
						transformerNotationFocus(),
						transformerNotationErrorLevel(),
						transformerMetaHighlight(),
					],
					defaultColor: false,
				},
			],
		],
	});
	return result.toString();
}

export function runMDX(code: string) {
	// Need to run sync so the server build also has full html
	// biome-ignore lint/suspicious/noExplicitAny: This is fine
	const mdx = runSync(code, runtime as any);
	console.log(mdx);
	const { default: Content, frontmatter } = mdx;

	return {
		Content,
		meta: frontmatter as {
			title?: string;
			description?: string;
			tags?: string[];
			date?: string;
			author?: string;
		},
	};
}
