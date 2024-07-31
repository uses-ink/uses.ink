import { headers } from "next/headers";

export type RepoRequest = {
	owner?: string;
	repo?: string;
	branch?: string;
	path?: string;
};

export const getRepoRequest = (): RepoRequest => {
	const headersList = headers();
	const host = headersList.get("host");
	const url = headersList.get("x-url");
	console.log("host", host);
	console.log("url", url);
	if (!url) {
		console.error("No url found in headers");
		return {};
	}
	const isLocalhost = host?.includes("localhost");
	const parts = host?.split(".") ?? [];
	let owner = undefined;
	if (isLocalhost) {
		if (parts.length > 1) {
			owner = parts[0];
		}
	} else if (parts.length > 2) {
		owner = parts[0];
	}
	let repo = undefined;
	let branch = undefined;
	let folder = undefined;

	const path = url.split(/https?:\/\/[^/]+/)[1];
	console.log("path", path);
	const pathParts = path.split("/").filter(Boolean);
	if (owner) {
		repo = pathParts.shift();
		console.log("repo", repo);

		if (repo?.includes("@")) {
			const [repoPart, branchPart] = repo.split("@");

			repo = repoPart;
			branch = branchPart;
		}
		folder = pathParts.join("/");
	} else {
		// If the owner is not defined, we ignre the repo and branch, and just use the path
		folder = path;
	}

	// <username>.uses.ink/[repo]@[branch]/[folder]
	// cestef.uses.ink/notes@main
	// cestef.uses.ink/notes/2021-09-01
	// cestef.uses.ink/notes@master/2021-09-01

	return {
		owner: owner,
		repo: repo,
		branch: branch,
		path: folder || undefined,
	};
};
