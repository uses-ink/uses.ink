import type { components, operations } from "@octokit/openapi-types";
import type { Meta } from "./schemas";

export type GithubTree =
	operations["git/get-tree"]["responses"]["200"]["content"]["application/json"];
export type GithubContent = components["schemas"]["content-file"];
export interface GitHubRequest {
	/** E.g. "thien-do" */
	owner: string;
	/** E.g. "notes" */
	repo: string;
	/** E.g. "dir/foo", "dir/foo/hello.md", "" when at repo root */
	path: string;
	/** E.g. "main", "feature/branch" */
	ref?: string;
}

export type RepoRequest = {
	owner?: string;
	repo?: string;
	ref?: string;
	path?: string;
};

export type GitHubContent =
	operations["repos/get-content"]["responses"]["200"]["content"]["application/json"];

export type GithubCommit =
	operations["repos/list-commits"]["responses"]["200"]["content"]["application/json"];

export type GitHubFile = components["schemas"]["content-file"];

export type GitHubDir = components["schemas"]["content-directory"];

export type GitHubEntry = GitHubDir[number];

export type DataResponse = {
	content: string;
	lastCommit?: CommitResponse;
	fileName: string;
};

export type CommitResponse = {
	date: string;
	author?: AuthorResponse;
	link?: string;
};

export type AuthorResponse = {
	name: string;
	login: string;
	avatar: string;
};

// Utility type to get the type of a promise
export type PromiseOf<T> = T extends Promise<infer U> ? U : T;

export enum FileType {
	Typst = "typst",
	Markdown = "markdown",
}
export const fileTypeFromExtension = (extension: string) => {
	switch (extension.toLowerCase().trim()) {
		case "md":
		case "markdown":
			return FileType.Markdown;
		case "typ":
		case "typst":
			return FileType.Typst;
		default:
			return null;
	}
};
export type CompileResult = {
	meta: Meta;
	runnable: string;
};

export type D2RenderResult = {
	svg: string;
	size: D2Size;
};
export type D2Size =
	| {
			height: number;
			width: number;
	  }
	| undefined;

export * from "./schemas";
