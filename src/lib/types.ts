import type { components, operations } from "@octokit/openapi-types";
import { z } from "zod";

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

export const MathEngineSchema = z.enum(["katex", "typst"]).default("typst");
export const LayoutSchema = z.enum(["post", "gallery"]).default("post");

export const ConfigSchema = z.object({
	hideTop: z.boolean().optional(),
	readingTime: z.boolean().optional(),
	mathEngine: MathEngineSchema.optional(),
	noHighlight: z.boolean().optional(),
	layout: LayoutSchema.optional(),
	nav: z.record(z.string()).optional(),
});

export const MetaSchema = z.object({
	title: z.string().optional(),
	description: z.string().optional(),
	date: z.string().optional(),
	author: z.string().optional(),
	image: z.string().optional(),
	...ConfigSchema.shape,
});

// Utility type to get the type of a promise
export type PromiseOf<T> = T extends Promise<infer U> ? U : T;

export const UserConfigSchema = z.object({
	defaultRepo: z.string().optional(),
	defaultBranch: z.string().optional(),
	...ConfigSchema.shape,
});
