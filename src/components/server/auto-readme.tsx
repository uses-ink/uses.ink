import type { RepoRequest } from "@/lib/server/repo-request";
import { DEFAULT_REPO } from "@/lib/constants";
import { fetchGithubTree } from "@/lib/server/github/tree";
import { dirname } from "node:path";
import Readme from "./readme";
import { serverLogger } from "@/lib/server/logger";

const AutoReadme = async ({
	repoRequest,
}: {
	repoRequest: RepoRequest;
}) => {
	const { owner, repo, branch, path } = repoRequest;
	const tree = await fetchGithubTree({
		// biome-ignore lint/style/noNonNullAssertion: this has been checked above
		owner: owner!,
		repo: repo ?? DEFAULT_REPO,
		path: path ?? "",
		branch,
	});
	// Filter markdown files in current directory
	const filteredTree = tree.tree.filter((file) => {
		const path = repoRequest.path ?? "";
		const isFile = file.type === "blob";
		const isMarkdown = file.path?.endsWith(".md");
		const isCurrentDir = dirname(file.path ?? "") === path;
		return isFile && isMarkdown && isCurrentDir;
	});
	serverLogger.debug({ filteredTree });
	return (
		<Readme {...{ repoRequest, filteredTree, path: repoRequest.path ?? "" }} />
	);
};

export default AutoReadme;
