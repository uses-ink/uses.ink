import type { GithubRequest, GithubTree, RepoRequest } from "@uses.ink/types";
import { DEFAULT_REF, DEFAULT_REPO, README_FILES } from "@uses.ink/constants";
import { FetchError } from "@uses.ink/errors";
import { logger } from "@uses.ink/server-logger";
import { filterTree, isReadmeRequest } from "./utils";
import { basename, join } from "node:path";

export const getRepoRequest = (
	host: string | null,
	urlString: string,
): RepoRequest => {
	const url = new URL(urlString);
	const isLocalhost = host?.includes("localhost");
	const parts = host?.split(".") ?? [];
	let owner: string | undefined = undefined;
	if (isLocalhost) {
		if (parts.length > 1) {
			owner = parts[0];
		}
	} else if (parts.length > 2) {
		owner = parts[0];
	}
	let repo: string | undefined = undefined;
	let ref: string | undefined = undefined;
	let folder: string | undefined = undefined;

	const path = url.pathname.replace(/^\//, "");
	const pathParts = path.split("/").filter(Boolean);
	if (owner) {
		repo = pathParts.shift();

		if (repo?.includes("@")) {
			const [repoPart, refPart] = repo.split("@");

			repo = repoPart;
			ref = refPart;
		}
		folder = pathParts.join("/");
	} else {
		// If the owner is not defined, we ignore the repo and ref, and just use the path
		folder = path;
	}

	// <username>.uses.ink/[repo]@[ref]/[folder]
	// cestef.uses.ink/notes@main
	// cestef.uses.ink/notes/2021-09-01
	// cestef.uses.ink/notes@master/2021-09-01
	return {
		owner,
		repo,
		ref: ref,
		remote: owner !== undefined,
		path: decodeURIComponent(folder) || undefined,
	} as any;
};

export const forgeGithubRequest = (repoRequest: RepoRequest): GithubRequest => {
	if (!repoRequest.remote) {
		throw Error("Cannot forge a GithubRequest from a local RepoRequest");
	}
	const { owner, repo, ref, path } = repoRequest;

	return {
		owner,
		repo: repo ?? DEFAULT_REPO,
		ref: ref ?? DEFAULT_REF,
		path: path ?? "",
	};
};

export const validateRequestAgainstTree = (
	request: GithubRequest,
	tree: GithubTree["tree"],
): string => {
	const path = request.path.trim() || ".";
	const filteredTree = filterTree(tree, path);

	logger.debug({ filteredTree: filteredTree.map((t) => t.path) });
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

export const validateRequestAgainstFiles = <T>(
	request: RepoRequest,
	files: Record<string, T>,
): [string, T] => {
	if (isReadmeRequest(request)) {
		const readme = Object.entries(files).find(([file]) =>
			README_FILES.some(
				(f) => basename(file).toLowerCase() === f.toLowerCase(),
			),
		);
		if (!readme) {
			throw new FetchError("NOT_FOUND_PREFETCH", 404);
		}
		return readme;
	}
	const path = request.path?.trim() || ".";

	const file = Object.entries(files).find(
		([file]) => basename(file).toLowerCase() === basename(path).toLowerCase(),
	);
	if (!file) {
		throw new FetchError("NOT_FOUND_PREFETCH", 404);
	}
	return file;
};
