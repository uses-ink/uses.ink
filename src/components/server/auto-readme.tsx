import type { RepoRequest } from "@/lib/types";
import { DEFAULT_REPO } from "@/lib/constants";
import { fetchGithubTree } from "@/lib/server/github/tree";
import { dirname } from "node:path";
import Readme from "./readme";
import { serverLogger } from "@/lib/server/logger";
import ErrorPage from "@/app/[[...slug]]/error";
import GetStartedPage from "./get-started";

const AutoReadme = async ({
	repoRequest,
}: {
	repoRequest: RepoRequest;
}) => {
	const { owner, repo, ref, path } = repoRequest;
	const tree = await fetchGithubTree({
		// biome-ignore lint/style/noNonNullAssertion: this has been checked above
		owner: owner!,
		repo: repo ?? DEFAULT_REPO,
		path: path ?? "",
		ref: ref ?? "HEAD",
	}).catch((error) => {
		serverLogger.error(error);
		return null;
	});

	if (!tree) {
		return <GetStartedPage {...{ repoRequest }} />;
	}
	// Filter markdown files in current directory
	const filteredTree = tree.tree.filter((file) => {
		const path = repoRequest.path ?? ".";
		const isFile = file.type === "blob";
		const isDirectory = file.type === "tree";
		const isMarkdown = file.path?.endsWith(".md");

		const isCurrentDir = dirname(file.path ?? ".") === path;
		return ((isFile && isMarkdown) || isDirectory) && isCurrentDir;
	});
	serverLogger.debug({ filteredTree });
	return (
		<Readme {...{ repoRequest, filteredTree, path: repoRequest.path ?? "" }} />
	);
};

export default AutoReadme;
