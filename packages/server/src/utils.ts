import { DEFAULT_REF, DEFAULT_REPO, EXTENSIONS } from "@uses.ink/constants";
import { logger } from "@uses.ink/server-logger/index.js";
import {
	FileType,
	type GithubRequest,
	type GithubTree,
	type RepoRequest,
} from "@uses.ink/types";
import { dirname, join } from "node:path";

export const isErrorHasStatus = (
	raw: unknown,
): raw is {
	status: number;
} => {
	if (typeof raw !== "object") return false;
	if (raw === null) throw raw;
	if ("status" in raw) {
		return typeof (raw as { status: number }).status === "number";
	}
	return false;
};

export const filterTree = (
	tree: GithubTree["tree"],
	basePath: string,
	extensions: string[] = EXTENSIONS,
) =>
	tree.filter((file) => {
		const isFile = file.type === "blob";
		const isFileType = extensions.some((ext) => file.path?.endsWith(`.${ext}`));

		const isCurrentDir = dirname(file.path ?? ".") === dirname(basePath);
		logger.debug("filterTree", {
			isFile,
			isFileType,
			isCurrentDir,
			basePath,
			at: file.path,
		});
		return isFile && isFileType && isCurrentDir;
	});

export const isReadmeRequest = (request: RepoRequest | GithubRequest) =>
	!request.path ||
	request.path === "." ||
	!EXTENSIONS.includes(request.path.split(".").pop() ?? "");

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

export const forgeUrlResolvers = (request: RepoRequest | GithubRequest) => ({
	asset: (url: string) => {
		if (request.owner) {
			const { owner, repo, path, ref } = request;
			const dir = dirname(path ?? "");
			const assetPath = join(dir, url);
			return `https://raw.githubusercontent.com/${owner}/${repo ?? DEFAULT_REPO}/${ref ?? DEFAULT_REF}/${assetPath}`;
		}
		return url;
	},
	link: (url: string) => {
		if (request.owner) {
			const { repo, path, ref } = request;
			const dir = dirname(path ?? "");
			const assetPath = join(dir, url);
			return `/${repo ?? DEFAULT_REPO}${ref && ref !== DEFAULT_REF ? `@${ref}` : ""}/${assetPath}`;
		}

		return url;
	},
});
