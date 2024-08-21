import type {
	GithubRequest,
	RepoRequest,
	RepoConfig,
	RenderResult,
} from "@uses.ink/types";
import { FileType } from "@uses.ink/types";
import { forgeGithubRequest, validateRequestAgainstTree } from "./requests";
import { fetchRepoConfig, fetchUserConfig } from "./configs";
import { fetchGithubTree } from "./github/tree";
import { fetchGithubRaw } from "./fetch";
import { EXTENSIONS } from "@uses.ink/constants";
import { fileTypeFromExtension, forgeUrlResolvers } from "./utils";
import { logger } from "@uses.ink/server-logger";
import { compileMarkdown } from "@uses.ink/render";

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
	return renderLocal(repoRequest.path);
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
	return renderContentByType(content, fileType, mergedConfig, request);
};
export const renderLocal = async (path: string): Promise<RenderResult> => {
	// TODO: Implement local rendering
	return {} as any;
};

export const renderContentByType = (
	content: string,
	fileType: FileType,
	config: RepoConfig,
	request: RepoRequest | GithubRequest,
): Promise<RenderResult> => {
	switch (fileType) {
		case FileType.Markdown:
			return renderMarkdown(content, config, request);
		case FileType.Typst:
			return renderTypst(content, config);
	}
};

export const renderMarkdown = async (
	content: string,
	config: RepoConfig,
	request: RepoRequest | GithubRequest,
): Promise<RenderResult> => {
	const urlResolvers = forgeUrlResolvers(request);
	const result = await compileMarkdown(content, urlResolvers, config);
	return { type: "markdown", result };
};

export const renderTypst = async (
	content: string,
	config: RepoConfig,
): Promise<RenderResult> => {
	//TODO: Implement Typst rendering
	return {} as any;
};
