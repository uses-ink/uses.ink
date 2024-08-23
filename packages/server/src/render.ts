import { EXTENSIONS } from "@uses.ink/constants";
import {
	compileMarkdownMDX,
	compileTypst,
	compileMarkdownMarked,
	compileMarkdownIt,
} from "@uses.ink/render";
import { logger } from "@uses.ink/server-logger";
import type {
	GithubRequest,
	RenderResult,
	RepoConfig,
	RepoRequest,
} from "@uses.ink/types";
import { FileType } from "@uses.ink/types";
import { ZodError } from "zod";
import { fetchRepoConfig, fetchUserConfig } from "./configs";
import { fetchGithubRaw } from "./fetch";
import { fetchGithubTree } from "./github/tree";
import {
	forgeGithubRequest,
	validateRequestAgainstFiles,
	validateRequestAgainstTree,
} from "./requests";
import {
	fileTypeFromExtension,
	filterTreeByPath,
	forgeUrlResolvers,
	isReadmeRequest,
	safeAsync,
	safeSync,
} from "./utils";

export const renderContent = async (
	repoRequest: RepoRequest,
): Promise<RenderResult> => {
	// If the request is remote, render remote
	if (repoRequest.remote) {
		// Forge github request
		const githubRequest = forgeGithubRequest(repoRequest);
		logger.debug("githubRequest", githubRequest);
		return await renderRemote(githubRequest);
	}
	// Else render local
	return await renderLocal(repoRequest);
};

export const renderRemote = async (
	githubRequest: GithubRequest,
): Promise<RenderResult> => {
	// Get user config
	const [userConfig, userConfigError] = await safeAsync(
		fetchUserConfig(githubRequest),
	);

	if (userConfigError) {
		logger.debug("userConfigError", userConfigError);
		if (userConfigError instanceof ZodError) {
			return {
				type: "error",
				payload: { error: userConfigError, request: githubRequest },
			};
		}
	}

	logger.debug("userConfig", userConfig);
	// Merge github request with user config
	const request: GithubRequest = {
		...githubRequest,
		...(userConfig?.defaultRepo && { repo: userConfig.defaultRepo }),
		...(userConfig?.defaultRef && { ref: userConfig.defaultRef }),
	};
	// Fetch github tree
	const [githubTree, treeError] = await safeAsync(fetchGithubTree(request));

	if (treeError) {
		logger.debug("treeError", treeError);
		if (treeError.name === "NOT_FOUND") {
			return { type: "get-started", payload: request };
		}
		return { type: "error", payload: { error: treeError, request } };
	}

	// Validate the request against the tree
	const [resolvedPath, validateError] = safeSync(() =>
		validateRequestAgainstTree(request, githubTree.tree),
	);

	if (validateError) {
		if (!isReadmeRequest(request)) {
			return { type: "not-found", payload: request };
		}
		logger.debug("validateError", validateError.name);
		logger.debug("rquest.path", request.path);
		const filteredTree = filterTreeByPath(githubTree.tree, request.path);
		logger.debug("filteredTree", filteredTree);
		return {
			type: "readme",
			payload: {
				tree: filteredTree,
				request,
			},
		};
	}

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
	const [repoConfig, repoConfigError] = await safeAsync(
		fetchRepoConfig(request),
	);

	if (repoConfigError) {
		if (repoConfigError.name === "NOT_FOUND") {
			logger.debug("repoConfigError", repoConfigError.message);
		} else if (repoConfigError instanceof ZodError) {
			return { type: "error", payload: { error: repoConfigError, request } };
		} else {
			logger.warn("unknown repoConfigError", repoConfigError.name);
			throw repoConfigError;
		}
	}

	// Merge the user config with the repo config
	const mergedConfig: RepoConfig = { ...userConfig, ...repoConfig };
	// Render the content
	return renderContentByType(content, fileType, request, mergedConfig);
};

export const renderLocal = async (
	request: RepoRequest,
): Promise<RenderResult> => {
	const files = import.meta.glob(
		"../../../apps/web/docs/**/*.{md,markdown,typ}",
		{ query: "raw", import: "default" },
	);
	logger.debug("files", files);
	const [resolvedPath, read] = validateRequestAgainstFiles(request, files);
	logger.debug("resolvedPath", resolvedPath);
	const extension = resolvedPath.split(".").pop() ?? "md";
	const fileType = fileTypeFromExtension(extension);

	// Check if the content is supported
	if (EXTENSIONS.indexOf(extension) === -1 || fileType === null) {
		throw new Error(`File type not supported: ${request.path}`);
	}

	const content = (await read()) as string;

	return renderContentByType(content, fileType, request);
};

export const renderContentByType = async (
	content: string,
	fileType: FileType,
	request: RepoRequest | GithubRequest,
	config?: RepoConfig,
): Promise<RenderResult> => {
	switch (fileType) {
		case FileType.Markdown: {
			// return renderMDX(content, request, config);
			// return renderMarked(content, request, config);
			// return renderMarkdownIt(content, request, config);
			// const renderers = [
			// 	{ name: "MDX", fn: renderMDX },
			// 	{ name: "Marked", fn: renderMarked },
			// 	{ name: "MarkdownIt", fn: renderMarkdownIt },
			// ];
			// const timings: { name: string; time: number }[] = [];
			// for (const renderer of renderers) {
			// 	const start = performance.now();
			// 	await renderer.fn(content, request, config);
			// 	timings.push({ name: renderer.name, time: performance.now() - start });
			// }
			// logger.debug("timings", timings);
			return renderMDX(content, request, config);
		}
		case FileType.Typst:
			return renderTypst(content, config);
	}
};

export const renderMDX = async (
	content: string,
	request: RepoRequest | GithubRequest,
	config?: RepoConfig,
): Promise<RenderResult> => {
	const urlResolvers = forgeUrlResolvers(request);
	const [result, resultError] = await safeAsync(
		compileMarkdownMDX(content, urlResolvers, config),
	);
	if (resultError) {
		if (resultError instanceof ZodError) {
			return { type: "error", payload: { error: resultError, request } };
		}
		throw resultError;
	}
	return { type: "mdx", payload: result };
};

export const renderMarked = async (
	content: string,
	request: RepoRequest | GithubRequest,
	config?: RepoConfig,
): Promise<RenderResult> => {
	const [result, resultError] = await safeAsync(
		compileMarkdownMarked(content, config),
	);
	if (resultError) {
		if (resultError instanceof ZodError) {
			return { type: "error", payload: { error: resultError, request } };
		}
		throw resultError;
	}
	return { type: "raw", payload: result };
};

export const renderMarkdownIt = async (
	content: string,
	request: RepoRequest | GithubRequest,
	config?: RepoConfig,
): Promise<RenderResult> => {
	const [result, resultError] = await safeAsync(
		compileMarkdownIt(content, config),
	);
	if (resultError) {
		if (resultError instanceof ZodError) {
			return { type: "error", payload: { error: resultError, request } };
		}
		throw resultError;
	}
	return { type: "raw", payload: result };
};

export const renderTypst = async (
	content: string,
	config?: RepoConfig,
): Promise<RenderResult> => {
	const result = await compileTypst(content, config);

	return { type: "typst", payload: { html: result } };
};
