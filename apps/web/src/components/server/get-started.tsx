import { DEFAULT_REPO } from "@/lib/constants";
import type { RepoRequest } from "@/lib/types";

const GITHUB_URL = "https://github.com";

const GetStartedPage = ({
	repoRequest,
}: {
	repoRequest: RepoRequest;
}) => {
	const { owner, repo } = repoRequest;
	const params = new URLSearchParams({
		name: repo ?? DEFAULT_REPO,
		owner: owner ?? "@me",
		description: "My very own blog!",
		visibility: "public",
	});
	const createRepoUrl = `${GITHUB_URL}/new?${params}`;

	return (
		<div className="flex w-screen h-[100svh] justify-center items-center prose max-w-full dark:prose-invert">
			<article>
				<h1 className="text-5xl">
					Welcome to <a href="https://uses.ink">uses.ink</a>
				</h1>
				<p className="text-2xl text-muted-foreground">
					There is nothing here <b>(yet!)</b>.
				</p>

				<p className="text-2xl text-muted-foreground">
					To get started, create a{" "}
					<a href={createRepoUrl} target="_blank" rel="noreferrer">
						GitHub repository
					</a>{" "}
					named <code>{repo ?? DEFAULT_REPO}</code> and add some markdown files
					to it.
				</p>
				<p className="text-2xl text-muted-foreground">
					You can also use the following URL to use another repository:
					<code>{"<owner>.uses.ink/<repo>"}</code>
				</p>
				<h3>
					<a href="https://uses.ink">Back to uses.ink</a>
				</h3>
			</article>
		</div>
	);
};

export default GetStartedPage;
