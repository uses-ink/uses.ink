import {
	DEFAULT_REF,
	DEFAULT_REPO,
	EXTENSIONS,
	README_FILES,
} from "@uses.ink/constants";
import { logger } from "@uses.ink/server-logger/index.js";
import type {
	HeadingMeta,
	Meta,
	GithubRequest,
	GithubTree,
	RepoRequest,
	ParsedGithubCommit,
} from "@uses.ink/types";
import { FileType } from "@uses.ink/types";
import { dirname, join, extname, basename } from "node:path";

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

		const isCurrentDir = dirname(file.path ?? ".") === dirname(basePath || ".");
		// logger.debug("filterTree", {
		// 	isFile,
		// 	isFileType,
		// 	isCurrentDir,
		// 	basePath,
		// 	at: file.path,
		// });
		return isFile && isFileType && isCurrentDir;
	});

export const filterTreeByPath = (tree: GithubTree["tree"], basePath: string) =>
	tree.filter((file) => {
		const isCurrentDir = dirname(file.path ?? ".") === (basePath || ".");
		// logger.debug("filterTree", {
		// 	isCurrentDir,
		// 	basePath,
		// 	at: file.path,
		// 	atDir: dirname(file.path ?? "."),
		// });
		return isCurrentDir;
	});

export const isReadmeRequest = (request: RepoRequest | GithubRequest) =>
	!request.path ||
	request.path === "." ||
	!EXTENSIONS.includes(extname(request.path).slice(1));

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
		logger.debug("forgeUrlResolvers.asset", { url, request });
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
			const isSupported =
				EXTENSIONS.includes(extname(assetPath).slice(1)) ||
				isReadmeRequest(request);
			if (!isSupported) {
				return `https://raw.githubusercontent.com/${request.owner}/${repo ?? DEFAULT_REPO}/${ref ?? DEFAULT_REF}/${assetPath}`;
			}
			return `/${repo ?? DEFAULT_REPO}${ref && ref !== DEFAULT_REF ? `@${ref}` : ""}/${assetPath}`;
		}

		return url;
	},
});

export const safeAsync = async <T, E extends Error = Error>(
	fnOrPromise: (() => Promise<T>) | Promise<T>,
): Promise<[T, null] | [null, E]> => {
	try {
		const result = await (typeof fnOrPromise === "function"
			? fnOrPromise()
			: fnOrPromise);
		return [result, null];
	} catch (error) {
		if (error instanceof Error) {
			return [null, error as E];
		}
		return [null, new Error("Unknown error occurred") as E];
	}
};

export const safeSync = <T, E extends Error = Error>(
	fnOrValue: (() => T) | T,
): [T, null] | [null, E] => {
	try {
		const result =
			typeof fnOrValue === "function" ? (fnOrValue as () => T)() : fnOrValue;
		return [result, null];
	} catch (error) {
		if (error instanceof Error) {
			return [null, error as E];
		}
		return [null, new Error("Unknown error occurred") as E];
	}
};

export const resolveMeta = (
	meta: Meta,
	request: RepoRequest,
	lastCommit?: ParsedGithubCommit,
	filename?: string,
): HeadingMeta => {
	const date = meta.date ?? lastCommit?.date;

	const author = lastCommit?.user
		? {
				name: lastCommit.user.name,
				link: `https://github.com/${lastCommit.user.login}`,
				avatar: lastCommit.user.avatar,
			}
		: { name: meta.author ?? request.owner };

	const title =
		meta.title ??
		(filename
			? README_FILES.includes(basename(filename.toLowerCase()))
				? `${request.owner}/${request.repo ?? DEFAULT_REPO}`
				: capitalizeFileName(filename)
			: `${request.owner}/${request.repo ?? DEFAULT_REPO}`);

	const description = meta.description;

	return {
		title,
		description,
		author,
		date,
	};
};

export const capitalizeFileName = (filename: string): string => {
	return filename
		.split(/[-_ ]/)
		.map((word) => word.charAt(0).toUpperCase() + word.slice(1))
		.join(" ");
};
