import { DEFAULT_META, EXTENSIONS } from "@uses.ink/constants";
import { RENDERER } from "astro:env/server";
import {
	compileMarkdownMDX,
	compileTypst,
	compileMarkdownMarked,
	compileMarkdownIt,
} from "@uses.ink/render";
import { logger } from "@uses.ink/server-logger";
import type {
	GithubRequest,
	Meta,
	ReadingTime,
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
import { fetchGithubLastCommit } from "./github/commit";
import { MetaSchema } from "@uses.ink/schemas/index.js";
import { readingTime as rT } from "reading-time-estimator";
import parseMatter from "gray-matter";
import {
	getCompileCache,
	setCompileCache,
	type MarkdownEngines,
} from "@uses.ink/cache";

// Helper function for merging configs
const mergeConfigs = (
	userConfig: RepoConfig | null,
	repoConfig: RepoConfig | null,
) => ({
	...userConfig,
	...repoConfig,
});

// Helper function to handle async errors and return formatted result
const handleError = (error: any, request: GithubRequest): RenderResult => {
	if (error instanceof ZodError) {
		return { type: "error", payload: { error, request } };
	}
	throw error;
};

// Common renderer handler
const renderRawWithCache = async (
	content: string,
	cacheKey: MarkdownEngines,
	renderFn: () => Promise<string>,
	meta: Meta,
	readingTime?: ReadingTime,
): Promise<RenderResult> => {
	const cached = await getCompileCache({ content, config: meta }, cacheKey);
	return {
		type: "raw",
		payload: {
			html:
				cached ||
				(await (async () => {
					const res = await renderFn();
					setCompileCache({ content, config: meta }, res, cacheKey);
					return res;
				})()),
			meta,
			readingTime,
		},
	};
};

export const renderContent = async (
	repoRequest: RepoRequest,
): Promise<RenderResult> => {
	return repoRequest.remote
		? renderRemote(forgeGithubRequest(repoRequest))
		: renderLocal(repoRequest);
};

export const renderRemote = async (
	githubRequest: GithubRequest,
): Promise<RenderResult> => {
	const userConfigStart = performance.now();
	const [userConfig, userConfigError] = await safeAsync(
		fetchUserConfig(githubRequest),
	);
	logger.debug(`fetchUserConfig took ${performance.now() - userConfigStart}ms`);
	if (userConfigError) {
		logger.debug("userConfigError", userConfigError.message);
	}

	const request = {
		...githubRequest,
		...(userConfig?.defaultRepo && { repo: userConfig.defaultRepo }),
		...(userConfig?.defaultRef && { ref: userConfig.defaultRef }),
	};
	const treeStart = performance.now();
	const [githubTree, treeError] = await safeAsync(fetchGithubTree(request));
	logger.debug(`fetchGithubTree took ${performance.now() - treeStart}ms`);

	if (treeError) {
		return treeError.name === "NOT_FOUND"
			? { type: "get-started", payload: request }
			: handleError(treeError, request);
	}

	const [resolvedPath, validateError] = safeSync(() =>
		validateRequestAgainstTree(request, githubTree.tree),
	);
	if (validateError) {
		if (!isReadmeRequest(request))
			return { type: "not-found", payload: request };
		const filteredTree = filterTreeByPath(githubTree.tree, request.path);
		return { type: "readme", payload: { tree: filteredTree, request } };
	}

	request.path = resolvedPath;
	const fileType = fileTypeFromExtension(resolvedPath.split(".").pop() ?? "md");

	if (!EXTENSIONS.includes(fileType!))
		throw new Error(`File type not supported: ${resolvedPath}`);

	const content = await fetchGithubRaw(request);
	const [repoConfig, repoConfigError] = await safeAsync(
		fetchRepoConfig(request),
	);

	if (repoConfigError) logger.debug("repoConfigError", repoConfigError.message);

	const mergedConfig = mergeConfigs(userConfig, repoConfig);
	const res = await renderContentByType(
		content,
		fileType!,
		request,
		mergedConfig,
	);
	const commit = await fetchGithubLastCommit(request);

	return { ...res, commit };
};

export const renderLocal = async (
	request: RepoRequest,
): Promise<RenderResult> => {
	const files = import.meta.glob(
		"../../../apps/web/docs/**/*.{md,markdown,typ}",
		{ query: "raw", import: "default" },
	);
	const [resolvedPath, read] = validateRequestAgainstFiles(request, files);

	const fileType = fileTypeFromExtension(resolvedPath.split(".").pop() ?? "md");
	if (!EXTENSIONS.includes(fileType!))
		throw new Error(`File type not supported: ${request.path}`);

	const content = (await read()) as string;
	return renderContentByType(content, fileType!, request);
};

export const renderContentByType = async (
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

const renderMarkdown = async (
	content: string,
	request: RepoRequest | GithubRequest,
	config?: RepoConfig,
): Promise<RenderResult> => {
	const matter = parseMatter(content);
	let [meta, metaError] = safeSync(() => MetaSchema.parse(matter.data));
	if (metaError) logger.debug("metaError", metaError);

	meta = { ...DEFAULT_META, ...config, ...meta };
	const readingTime = meta.readingTime ? rT(matter.content) : undefined;

	const renderMap = {
		marked: renderMarked,
		markdownIt: renderMarkdownIt,
		mdx: renderMDX,
	};
	const renderer = renderMap[RENDERER] || null;
	if (!renderer) throw new Error(`Unknown renderer: ${RENDERER}`);

	return renderer(matter.content, request, meta, readingTime);
};

const renderMDX = async (
	content: string,
	request: RepoRequest | GithubRequest,
	meta: Meta,
	readingTime?: ReadingTime,
): Promise<RenderResult> => {
	const cached = await getCompileCache({ content, config: meta }, "mdx");
	return {
		type: "mdx",
		payload: {
			runnable:
				cached ||
				(await (async () => {
					const res = await compileMarkdownMDX(
						content,
						forgeUrlResolvers(request),
						meta,
					);
					setCompileCache({ content, config: meta }, res, "mdx");
					return res;
				})()),
			meta,
			readingTime,
		},
	};
};

const renderMarked = async (
	content: string,
	request: RepoRequest | GithubRequest,
	meta: Meta,
	readingTime?: ReadingTime,
): Promise<RenderResult> =>
	renderRawWithCache(
		content,
		"marked",
		() => compileMarkdownMarked(content, meta),
		meta,
		readingTime,
	);

const renderMarkdownIt = async (
	content: string,
	request: RepoRequest | GithubRequest,
	meta: Meta,
	readingTime?: ReadingTime,
): Promise<RenderResult> =>
	renderRawWithCache(
		content,
		"markdownIt",
		() => compileMarkdownIt(content, meta),
		meta,
		readingTime,
	);

//TODO: Implement cache for typst
export const renderTypst = async (
	content: string,
	config?: RepoConfig,
): Promise<RenderResult> => ({
	type: "typst",
	payload: { html: await compileTypst(content, config) },
});
