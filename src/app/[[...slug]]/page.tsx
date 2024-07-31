import { Footer } from "@/components/footer";
import Post from "@/components/post";
import { RepoDevTools } from "@/components/repo";
import {
	DEFAULT_REPO,
	EXTENSIONS,
	IS_DEV,
	SHOW_DEV_TOOLS,
} from "@/lib/constants";
import { fetchConfig, fetchData, fetchLocalData } from "@/lib/fetch";
import { compileMDX } from "@/lib/mdx";
import { getRepoRequest } from "@/lib/repo-request";
import type { GitHubRequest } from "@/lib/types";
import type { NextPage } from "next";
import { dirname, join } from "node:path";
import { ZodError } from "zod";
import ErrorPage from "./error";
import { FetchError } from "@/lib/errors";
import { fetchGithubTree } from "@/lib/github/tree";
import Readme from "@/components/readme";

const Page: NextPage = async () => {
	const repoRequest = getRepoRequest();
	console.log("repoRequest", JSON.stringify(repoRequest));

	const isRemote = !!repoRequest.owner;

	try {
		const { content, lastCommit, fileName } = isRemote
			? await fetchData({
					...repoRequest,
					repo: repoRequest.repo ?? DEFAULT_REPO,
					path: repoRequest.path ?? "",
				} as GitHubRequest)
			: await fetchLocalData(repoRequest.path ?? "README.md");

		const extension = fileName.split(".").pop() ?? "md";

		if (EXTENSIONS.indexOf(extension) === -1) {
			return (
				<ErrorPage repoData={repoRequest} error="File type not supported" />
			);
		}

		const config = isRemote
			? await fetchConfig({
					...repoRequest,
					repo: repoRequest.repo ?? DEFAULT_REPO,
					path: repoRequest.path ?? "",
				} as GitHubRequest)
			: null;

		console.log("config", config);

		const res = await compileMDX(content, {
			asset: (url) => {
				if (repoRequest.owner) {
					const { owner, repo, path, branch } = repoRequest;
					const dir = dirname(path ?? "");
					const assetPath = join(dir, url);
					const origin = "https://raw.githubusercontent.com";
					return `${origin}/${owner}/${repo ?? DEFAULT_REPO}/${branch ?? "HEAD"}/${assetPath}`;
				}
				return url;
			},
			link: (url) => {
				if (repoRequest.owner) {
					const { repo, path, branch } = repoRequest;
					const dir = dirname(path ?? "");
					const assetPath = join(dir, url);
					return `/${repo ?? DEFAULT_REPO}${branch ? `@${branch}` : ""}/${assetPath}`;
				}

				return url;
			},
		});

		return (
			<article
				className="container mx-auto xl:prose-lg prose max-md:prose-sm dark:prose-invert"
				dir="ltr"
			>
				{IS_DEV && SHOW_DEV_TOOLS && repoRequest !== null && (
					<RepoDevTools {...repoRequest} />
				)}

				{res instanceof ZodError ? (
					<FrontmatterError errors={res.errors} />
				) : (
					<Post
						runnable={res.runnable}
						meta={res.meta}
						lastCommit={lastCommit}
						filename={repoRequest.path}
						config={config}
						request={repoRequest}
					/>
				)}
				<hr className="!mb-4" />
				<Footer />
			</article>
		);
	} catch (error) {
		console.log("Error", error);
		console.log("repoData", repoRequest);

		if (error instanceof FetchError) {
			const isReadmeRequest =
				!repoRequest.path ||
				!EXTENSIONS.includes(repoRequest.path.split(".").pop() ?? "");
			console.log("isReadmeRequest", isReadmeRequest);
			if (error.name === "NOT_FOUND" && isReadmeRequest && isRemote) {
				// Try to generate a README ourselves
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
				console.log("filteredTree", filteredTree);
				return (
					<Readme
						{...{ repoRequest, filteredTree, path: repoRequest.path ?? "" }}
					/>
				);
			}
			return <ErrorPage repoData={repoRequest} error={error.message} />;
		}

		return <ErrorPage repoData={repoRequest} error="Unknown error" />;
	}
};

export default Page;
