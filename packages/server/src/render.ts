import type {
	GithubRequest,
	RepoRequest,
	RepoConfig,
	RenderResult,
} from "@uses.ink/types";
import { FileType } from "@uses.ink/types";
import {
	forgeGithubRequest,
	validateRequestAgainstFiles,
	validateRequestAgainstTree,
} from "./requests";
import { fetchRepoConfig, fetchUserConfig } from "./configs";
import { fetchGithubTree } from "./github/tree";
import { fetchGithubRaw } from "./fetch";
import { EXTENSIONS } from "@uses.ink/constants";
import { fileTypeFromExtension, forgeUrlResolvers } from "./utils";
import { logger } from "@uses.ink/server-logger";
import { compileMarkdown, compileTypst } from "@uses.ink/render";
import { readdir, readFile } from "node:fs/promises";
import { join } from "node:path";

export const renderContent = async (
	repoRequest: RepoRequest,
): Promise<RenderResult> => {
	// If the request is remote, render remote
	if (repoRequest.remote) {
		// Forge github request
		const githubRequest = forgeGithubRequest(repoRequest);
		logger.debug("githubRequest", githubRequest);
		return renderRemote(githubRequest);
	}
	// Else render local
	return renderLocal(repoRequest);
};

export const renderRemote = async (
	githubRequest: GithubRequest,
): Promise<RenderResult> => {
	// Get user config
	const userConfig = await fetchUserConfig(githubRequest);
	logger.debug("userConfig", userConfig);
	// Merge github request with user config
	const request: GithubRequest = {
		...githubRequest,
		...(userConfig.defaultRepo && { repo: userConfig.defaultRepo }),
		...(userConfig.defaultRef && { ref: userConfig.defaultRef }),
	};
	// Fetch github tree
	const { tree } = await fetchGithubTree(request);
	// Validate the request against the tree
	const resolvedPath = validateRequestAgainstTree(request, tree);

	const extension = resolvedPath.split(".").pop() ?? "md";
	const fileType = fileTypeFromExtension(extension);

	// Check if the content is supported
	if (EXTENSIONS.indexOf(extension) === -1 || fileType === null) {
		throw new Error(`File type not supported: ${resolvedPath}`);
	}

	request.path = resolvedPath;

	// Fetch the content
	const content = await fetchGithubRaw(request);
	// Fetch the repo config
	const repoConfig = await fetchRepoConfig(request);
	// Merge the user config with the repo config
	const mergedConfig: RepoConfig = { ...userConfig, ...repoConfig };
	// Render the content
	return renderContentByType(content, fileType, request, mergedConfig);
};
export const renderLocal = async (
	request: RepoRequest,
): Promise<RenderResult> => {
	const files = await readdir(join(process.cwd(), "docs"));
	logger.debug("files", files);
	const resolvedPath = validateRequestAgainstFiles(request, files);
	const extension = resolvedPath.split(".").pop() ?? "md";
	const fileType = fileTypeFromExtension(extension);

	// Check if the content is supported
	if (EXTENSIONS.indexOf(extension) === -1 || fileType === null) {
		throw new Error(`File type not supported: ${request.path}`);
	}

	const content = await readFile(
		join(process.cwd(), "docs", resolvedPath),
		"utf-8",
	);

	return renderContentByType(content, fileType, request);
};

export const renderContentByType = (
	content: string,
	fileType: FileType,
	request: RepoRequest | GithubRequest,
	config?: RepoConfig,
): Promise<RenderResult> => {
	switch (fileType) {
		case FileType.Markdown:
			return renderMarkdown(content, request, config);
		case FileType.Typst:
			return renderTypst(content, config);
	}
};

export const renderMarkdown = async (
	content: string,
	request: RepoRequest | GithubRequest,
	config?: RepoConfig,
): Promise<RenderResult> => {
	const urlResolvers = forgeUrlResolvers(request);
	const result = await compileMarkdown(content, urlResolvers, config);
	return { type: "markdown", result };
};

export const renderTypst = async (
	content: string,
	config?: RepoConfig,
): Promise<RenderResult> => {
	const result = await compileTypst(content, config);

	return { type: "typst", result: { html: result } };
};
