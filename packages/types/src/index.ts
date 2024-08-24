import type { GithubRequest, GithubTree, ParsedGithubCommit } from "./github";
import type { Meta } from "./schemas";
import type { FetchError } from "@uses.ink/errors";

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
export type ReadingTime = {
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
export type MDXCompileResult = {
	meta: Meta;
	runnable: string;
	readingTime?: ReadingTime;
};

export type MarkdownRawCompileResult = {
	meta: Meta;
	readingTime?: ReadingTime;
	html: string;
};

export type TypstCompileResult = {
	html: string;
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

export type HeadingMeta = {
	title: string;
	description?: string;
	author: { name?: string; link?: string; avatar?: string };
	date?: string;
	dateLink?: string;
};

export type RenderResult = (
	| {
			type: "typst";
			payload: TypstCompileResult;
	  }
	| {
			type: "mdx";
			payload: MDXCompileResult;
	  }
	| {
			type: "raw";
			payload: MarkdownRawCompileResult;
	  }
	| {
			type: "readme";
			payload: {
				tree: GithubTree["tree"];
				request: GithubRequest;
			};
	  }
	| {
			type: "get-started";
			payload: GithubRequest;
	  }
	| {
			type: "error";
			payload: {
				error: FetchError | Error;
				request: RepoRequest | GithubRequest;
			};
	  }
	| {
			type: "not-found";
			payload: GithubRequest | RepoRequest;
	  }
) & { commit?: ParsedGithubCommit };

export * from "./github";
export * from "./schemas";
