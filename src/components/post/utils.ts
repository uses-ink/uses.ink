import { runMDX } from "@/lib/mdx/run";
import { mdxComponents } from "@/lib/mdx/components";
import { useState, useEffect } from "react";

export const useRunMDX = (code: string) => {
	const [result, setResult] = useState<ReturnType<typeof runMDX>>();
	useEffect(() => {
		setResult(runMDX(code));
	}, [code]);
	return result ?? { Content: null, meta: null, readingTime: {} };
};

export const resolveTitle = (Content: (props: any) => any) => {
	const content = Content({ components: mdxComponents })?.props.children ?? [];
	if (Symbol.iterator in Object(content)) {
		for (const child of content) {
			if (["h1", "h2", "h3"].includes(child.type)) {
				return child.props.children;
			}
		}
	} else {
		if (["h1", "h2", "h3"].includes(content.type)) {
			return content.props;
		}
	}
	return null;
};
