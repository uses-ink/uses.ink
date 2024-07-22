import Pre from "@/components/pre";
import Head from "next/head";
import Link from "next/link";

// To have client-side routing
const CustomLink = (props: any) => <Link {...props} />;

/**
 * Use Next's components in MDX (e.g. Head)
 */
export const mdxComponents = {
	a: CustomLink,
	// https://github.com/mdx-js/mdx/discussions/1921
	head: Head as any,
	pre: Pre as any,
};
