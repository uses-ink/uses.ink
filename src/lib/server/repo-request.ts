import { headers } from "next/headers";
import { serverLogger } from "./logger";
import type { RepoRequest } from "../types";

export const getRepoRequest = (): {
	req: RepoRequest;
	host?: string;
	url: URL;
} => {
	const headersList = headers();
	const host = headersList.get("host");
	// biome-ignore lint/style/noNonNullAssertion:  This is set in the middleware
	const url = new URL(headersList.get("x-url")!);
	serverLogger.debug({ host, url });
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
	let ref = undefined;
	let folder = undefined;

	const path = url.pathname.replace(/^\//, "");
	serverLogger.debug({ path });
	const pathParts = path.split("/").filter(Boolean);
	if (owner) {
		repo = pathParts.shift();
		serverLogger.debug({ repo });

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
		req: {
			owner: owner,
			repo: repo,
			ref: ref,
			path: folder || undefined,
		},
		host: host ?? undefined,
		url,
	};
};
