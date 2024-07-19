import CodeBlock from "@/components/pre";
import Head from "next/head";
import Link from "next/link";

// To have client-side routing
// biome-ignore lint/suspicious/noExplicitAny: <explanation>
const CustomLink = (props: any) => <Link {...props} />;

/**
 * Use Next's components in MDX (e.g. Head)
 */
export const mdxComponents = {
	a: CustomLink,
	// https://github.com/mdx-js/mdx/discussions/1921
	// biome-ignore lint/suspicious/noExplicitAny: <explanation>
	head: Head as any,
	// biome-ignore lint/suspicious/noExplicitAny: <explanation>
	pre: CodeBlock as any,
};
