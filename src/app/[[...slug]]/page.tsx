import { dirname, join } from "node:path";
import ErrorPage from "./error";
import { Footer } from "@/components/footer";
import Post from "@/components/post";
import { RepoDevTools } from "@/components/repo";
import { DEFAULT_REPO, SHOW_DEV_TOOLS } from "@/lib/constants";
import { fetchConfig, fetchData, fetchLocalData } from "@/lib/fetch";
import { compileMDX } from "@/lib/mdx";
import { getRepoRequest } from "@/lib/repo-request";
import { MetaSchema, type GitHubRequest } from "@/lib/types";
import type { NextPage } from "next";

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

	const config = isRemote
		? await fetchConfig({
				...repoRequest,
				repo: repoRequest.repo ?? DEFAULT_REPO,
				path: repoRequest.path ?? "",
			} as GitHubRequest)
		: null;

	console.log("config", config);

	const extension = fileName.split(".").pop() ?? "md";

	if (extension !== "md") {
		return <ErrorPage repoData={repoRequest} error="File type not supported" />;
	}

	const { runnable, meta } = await compileMDX(content, {
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

	const parsedMeta = MetaSchema.safeParse(meta);

	return (
		<article
			className="container mx-auto xl:prose-lg prose max-md:prose-sm dark:prose-invert"
			dir="ltr"
		>
			{isDev && SHOW_DEV_TOOLS && repoRequest !== null && (
				<RepoDevTools {...repoRequest} />
			)}

			<Post
				runnable={runnable}
				meta={parsedMeta}
				lastCommit={lastCommit}
				filename={repoRequest.path}
				config={config}
			/>
			<hr className="!mb-4" />
			<Footer />
		</article>
	);
};

export default Page;
