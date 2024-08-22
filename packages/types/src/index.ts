import type { Meta } from "./schemas";

export enum FileType {
	Typst = "typst",
	Markdown = "markdown",
}
export type RepoRequest =
	| { owner: string; repo?: string; ref?: string; path?: string; remote: true }
	| {
			owner?: undefined;
			repo?: undefined;
			ref?: undefined;
			path: string;
			remote: false;
	  };
type ReadingTime = {
	/**
	 * Number of minutes to read the text
	 */
	readonly minutes: number;
	/**
	 * Number of words in the text
	 */
	readonly words: number;
	/**
	 * Localized message with the number of minutes to read the text
	 */
	readonly text: string;
};
export type MarkdownCompileResult = {
	meta: Meta;
	runnable: string;
	readingTime?: ReadingTime;
};

export type D2Size =
	| {
			height: number;
			width: number;
	  }
	| undefined;
export type D2RenderResult = {
	svg: string;
	size: D2Size;
};

export type RenderResult =
	| {
			type: "typst";
			result: {
				html: string;
			};
	  }
	| {
			type: "markdown";
			result: MarkdownCompileResult;
	  };

export * from "./github";
export * from "./schemas";
