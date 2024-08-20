import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
	return twMerge(clsx(inputs));
}

import { mdxComponents } from "./mdx/components";
import { clientLogger } from "@uses.ink/logger";

export function flipImageSource(theme: string) {
	clientLogger.log("theme -> flipImageSource", theme);
	// clientLogger.debug(
	// 	"document.querySelectorAll(picture)",
	// 	document.querySelectorAll("picture"),
	// );

	// biome-ignore lint/complexity/noForEach: cannot iterate over NodeList
	document.querySelectorAll("picture").forEach((element) => {
		const img = element.querySelector("img");
		const darkSource = element.querySelector(
			"source[media='(prefers-color-scheme: dark)']",
		) as HTMLSourceElement | undefined;
		clientLogger.debug({ darkSource, img });
		if (!darkSource || !img) {
			console.error("darkSource or img is undefined");
			return;
		}

		let lightSource = element.querySelector(
			"source[media='(prefers-color-scheme: light)']",
		) as HTMLSourceElement | undefined;
		if (!lightSource) {
			clientLogger.debug("creating lightSource");
			lightSource = document.createElement("source");
			lightSource.media = "(prefers-color-scheme: light)";
			lightSource.srcset = img.src;
			lightSource.width = img.width;
			lightSource.height = img.height;
			element.prepend(lightSource);
		} else {
			clientLogger.debug("lightSource already exists");
		}

		if (theme === "dark") {
			clientLogger.debug("dark theme");
			img.src = darkSource.srcset;
			img.className = "d2-dark";
		} else {
			clientLogger.debug("light theme");
			img.src = lightSource.srcset;
			img.className = "d2-light";
		}
	});
}
export const resolveTitle = (
	Content: (props: any) => any,
): string | undefined => {
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
	return;
};

export const userContentHash = () => {
	const handleHashChange = () => {
		let hash: string;

		try {
			hash = decodeURIComponent(location.hash.slice(1)).toLowerCase();
		} catch (e) {
			clientLogger.warn("Error decoding hash", e);
			return;
		}

		const name = `user-content-${hash}`;
		const target =
			document.getElementById(name) ||
			document.getElementsByName(name)[0] ||
			document.getElementById(hash);

		if (target) {
			clientLogger.debug("Post -> target", target);

			const offset = 100;
			const y = target.getBoundingClientRect().top + window.scrollY - offset;
			window.scrollTo({ top: y, behavior: "smooth" });
		} else {
			clientLogger.warn("No anchor found for:", hash);
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
