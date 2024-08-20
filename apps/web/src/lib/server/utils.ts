import { basename, dirname, join } from "node:path";

import { DEFAULT_REPO, EXTENSIONS, README_FILES } from "@uses.ink/constants";
import type {
	CommitResponse,
	GitHubRequest,
	GithubTree,
	Meta,
	RepoRequest,
} from "@uses.ink/types";
import { capitalizeFileName } from "../client/utils";

import { FetchError } from "@uses.ink/errors";
import { serverLogger } from "@uses.ink/logger";

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

export type ResolvedMetadata = {
	title: string;
	description?: string;
	author: { name?: string; link?: string; avatar?: string };
	date?: string;
};

export const resolveMetadata = (
	meta: Meta,
	// Content: React.FC,
	request: RepoRequest,
	lastCommit?: CommitResponse,
	filename?: string,
): ResolvedMetadata => {
	const date = meta.date ?? lastCommit?.date;

	const author = lastCommit?.author
		? {
				name: lastCommit.author.name,
				link: `https://github.com/${lastCommit.author.login}`,
				avatar: lastCommit.author.avatar,
			}
		: { name: meta.author };

	const title =
		meta.title ??
		// resolveTitle(Content) ??
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

export const validateRequestAgainstTree = (
	request: GitHubRequest,
	{ tree }: GithubTree,
): string => {
	const path = request.path.trim() || ".";
	const filteredTree = filterTree(tree, path);

	serverLogger.debug({ filteredTree: filteredTree.map((t) => t.path) });
	if (isReadmeRequest(request)) {
		const readme = filteredTree.find((file) =>
			README_FILES.some(
				(f) => basename(file.path ?? "").toLowerCase() === f.toLowerCase(),
			),
		);
		if (!readme) {
			throw new FetchError("NOT_FOUND_PREFETCH", 404);
		}
		return readme.path ?? join(path, "README.md");
	}
	const file = filteredTree.find(
		(file) =>
			basename(file.path ?? "").toLowerCase() ===
			basename(request.path).toLowerCase(),
	);
	if (!file) {
		throw new FetchError("NOT_FOUND_PREFETCH", 404);
	}
	return file.path ?? join(path, request.path);
};

export const filterTree = (
	tree: GithubTree["tree"],
	basePath: string,
	extensions: string[] = EXTENSIONS,
) =>
	tree.filter((file) => {
		const isFile = file.type === "blob";
		const isFileType = extensions.some((ext) => file.path?.endsWith(`.${ext}`));

		const isCurrentDir = dirname(file.path ?? ".") === basePath;
		return isFile && isFileType && isCurrentDir;
	});

export const isReadmeRequest = (request: RepoRequest) =>
	!request.path ||
	request.path === "." ||
	!EXTENSIONS.includes(request.path.split(".").pop() ?? "");
