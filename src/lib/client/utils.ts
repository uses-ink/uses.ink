import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
	return twMerge(clsx(inputs));
}

import { mdxComponents } from "@/lib/client/mdx/components";

export const resolveTitle = (Content: (props: any) => any): string | null => {
	const content = Content({ components: mdxComponents })?.props.children ?? [];
	if (Symbol.iterator in Object(content)) {
		for (const child of content) {
			if (["h1", "h2", "h3"].includes(child.type)) {
				if (typeof child.props.children === "string") {
					return child.props.children;
				}
			}
		}
	} else {
		if (["h1", "h2", "h3"].includes(content.type)) {
			if (typeof content.props.children === "string") {
				return content.props;
			}
		}
	}
	return null;
};

export const userContentHash = () => {
	const handleHashChange = () => {
		let hash: string;

		try {
			hash = decodeURIComponent(location.hash.slice(1)).toLowerCase();
		} catch (e) {
			console.warn("Error decoding hash", e);
			return;
		}

		const name = `user-content-${hash}`;
		const target =
			document.getElementById(name) ||
			document.getElementsByName(name)[0] ||
			document.getElementById(hash);

		if (target) {
			console.log("Post -> target", target);

			const offset = 100;
			const y = target.getBoundingClientRect().top + window.scrollY - offset;
			window.scrollTo({ top: y, behavior: "smooth" });
		} else {
			console.warn("No anchor found for:", hash);
		}
	};
	const handleClick = (event: MouseEvent) => {
		if (
			event.target &&
			event.target instanceof HTMLAnchorElement &&
			event.target.href === location.href &&
			location.hash.length > 1
		) {
			if (!event.defaultPrevented) {
				handleHashChange();
			}
		}
	};

	handleHashChange();

	return { handleClick, handleHashChange };
};

export const capitalizeFileName = (filename: string): string => {
	return filename
		.split(/[-_ ]/)
		.map((word) => word.charAt(0).toUpperCase() + word.slice(1))
		.join(" ");
};
