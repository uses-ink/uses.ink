import { type ClassValue, clsx } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
	return twMerge(clsx(inputs));
}

export function syntaxHighlightJSON(json: string) {
	const newJson = json
		.replace(/&/g, "&amp;")
		.replace(/</g, "&lt;")
		.replace(/>/g, "&gt;");
	return newJson.replace(
		/("(\\u[a-zA-Z0-9]{4}|\\[^u]|[^\\"])*"(\s*:)?|\b(true|false|null)\b|-?\d+(?:\.\d*)?(?:[eE][+\-]?\d+)?)/g,
		(match) => {
			let cls = "--shiki-token-constant";
			if (/^"/.test(match)) {
				if (/:$/.test(match)) {
					cls = "--shiki-token-keyword";
				} else {
					cls = "--shiki-token-string";
				}
			} else if (/true|false/.test(match)) {
				cls = "--shiki-token-constant";
			} else if (/null/.test(match)) {
				cls = "--shiki-token-function";
			}
			return `<span style="color:var(${cls});">${match}</span>`;
		},
	);
}
