import type { RepoRequest } from "@/lib/types";
import { DEFAULT_REF, DEFAULT_REPO } from "@/lib/constants";
import { fetchGithubTree, type GithubTree } from "@/lib/server/github/tree";
import { dirname } from "node:path";
import Readme from "./readme";
import { serverLogger } from "@/lib/server/logger";
import ErrorPage from "@/app/[[...slug]]/error";
import GetStartedPage from "./get-started";

const AutoReadme = async ({
	repoRequest,
	tree,
}: {
	repoRequest: RepoRequest;
	tree?: GithubTree;
}) => {
	if (!tree) {
		return <GetStartedPage {...{ repoRequest }} />;
	}
	// Filter markdown files in current directory
	const path = repoRequest.path?.trim() || ".";
	const filteredTree = tree.tree.filter((file) => {
		const isFile = file.type === "blob";
		const isDirectory = file.type === "tree";
		const isMarkdown = file.path?.endsWith(".md");

		const isCurrentDir = dirname(file.path ?? ".") === path;
		return ((isFile && isMarkdown) || isDirectory) && isCurrentDir;
	});
	// serverLogger.debug({ filteredTree });
	return (
		<Readme {...{ repoRequest, filteredTree, path: repoRequest.path ?? "" }} />
	);
};

export default AutoReadme;
