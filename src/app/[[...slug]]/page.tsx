import { Footer } from "@/components/footer";
import Post from "@/components/post";
import { RepoDevTools } from "@/components/repo";
import { DEFAULT_REPO, EXTENSIONS, SHOW_DEV_TOOLS } from "@/lib/constants";
import { fetchConfig, fetchData, fetchLocalData } from "@/lib/fetch";
import { compileMDX } from "@/lib/mdx";
import { getRepoRequest } from "@/lib/repo-request";
import type { GitHubRequest } from "@/lib/types";
import type { NextPage } from "next";
import { dirname, join } from "node:path";
import { ZodError } from "zod";
import ErrorPage from "./error";

const isDev = process.env.NODE_ENV === "development";

const Page: NextPage = async () => {
	const repoRequest = getRepoRequest();
	console.log("repoRequest", JSON.stringify(repoRequest));

	const isRemote = !!repoRequest.owner;

	const { content, lastCommit, error, fileName } = isRemote
		? await fetchData({
				...repoRequest,
				repo: repoRequest.repo ?? DEFAULT_REPO,
				path: repoRequest.path ?? "",
			} as GitHubRequest)
		: await fetchLocalData(repoRequest.path ?? "README.md");

	if (error !== undefined) {
		console.log("Error", error);
		console.log("repoData", repoRequest);

		return <ErrorPage repoData={repoRequest} error={error} />;
	}

	const extension = fileName.split(".").pop() ?? "md";

	if (EXTENSIONS.indexOf(extension) === -1) {
		return <ErrorPage repoData={repoRequest} error="File type not supported" />;
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
			{isDev && SHOW_DEV_TOOLS && repoRequest !== null && (
				<RepoDevTools {...repoRequest} />
			)}

			{res instanceof ZodError ? (
				<div className="flex w-screen justify-center items-center prose max-w-full dark:prose-invert">
					<div className="text-center flex gap-2 flex-col items-center">
						<h1 className="text-4xl">
							An error occured when parsing your frontmatter.
						</h1>
						<pre className="text-left language-json">
							{JSON.stringify(res.errors, null, 2)}
						</pre>
						<p className="text-lg dark:text-gray-400 text-gray-600">
							See the{" "}
							<a
								href="https://uses.ink/docs/frontmatter"
								target="_blank"
								rel="noreferrer"
							>
								frontmatter documentation
							</a>{" "}
							for more information.
						</p>
						<h3>
							<a href="https://uses.ink">Back to uses.ink</a>
						</h3>
					</div>
				</div>
			) : (
				<Post
					runnable={res.runnable}
					meta={res.meta}
					lastCommit={lastCommit}
					filename={repoRequest.path}
					config={config}
				/>
			)}
			<hr className="!mb-4" />
			<Footer />
		</article>
	);
};

export default Page;
