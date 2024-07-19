"use client";

import { runMDX } from "@/lib/mdx";
import { mdxComponents } from "@/lib/mdx/components";
import { useEffect } from "react";

export default function Post({ content }: { content: string }) {
	const { meta, Content } = runMDX(content);
	useEffect(() => {
		if (meta?.title) {
			document.title = meta.title;
		}
		if (meta?.description) {
			document
				.querySelector('meta[name="description"]')
				?.setAttribute("content", meta.description);
		}
	}, [meta]);
	return (
		<>
			<article className="prose md:prose-lg dark:prose-invert">
				<Content components={mdxComponents} />
			</article>
		</>
	);
}
