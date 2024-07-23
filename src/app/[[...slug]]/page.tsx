import { dirname, join } from "node:path";
import ErrorPage from "./error";
import { Footer } from "@/components/footer";
import Post from "@/components/post";
import { RepoDevTools } from "@/components/repo";
import { DEFAULT_REPO, SHOW_DEV_TOOLS } from "@/lib/constants";
import { fetchConfig, fetchData, fetchLocalData } from "@/lib/fetch";
import { compileMDX } from "@/lib/mdx";
import { getRepo } from "@/lib/repo";
import type { GitHubRequest } from "@/lib/types";
import type { NextPage } from "next";

const isDev = process.env.NODE_ENV === "development";

const Page: NextPage = async () => {
	const repoData = getRepo();
	console.log("repoData", JSON.stringify(repoData));

	const { content, lastCommit, error, fileName } =
		repoData.owner !== null
			? await fetchData({
					...repoData,
					repo: repoData.repo ?? DEFAULT_REPO,
					path: repoData.path ?? "",
				} as GitHubRequest)
			: await fetchLocalData(repoData.path ?? "README.md");

	if (error !== undefined) {
		console.log("Error", error);
		console.log("repoData", repoData);

		return <ErrorPage repoData={repoData} error={error} />;
	}
	const config =
		repoData.owner !== null
			? await fetchConfig({
					...repoData,
					repo: repoData.repo ?? DEFAULT_REPO,
					path: repoData.path ?? "",
				} as GitHubRequest)
			: null;

	console.log("config", config);

	const extension = fileName.split(".").pop() ?? "md";

	if (extension !== "md") {
		return <ErrorPage repoData={repoData} error="File type not supported" />;
	}

	const mdx = await compileMDX(content, {
		asset: (url) => {
			if (repoData.owner) {
				const { owner, repo, path, branch } = repoData;
				const dir = dirname(path ?? "");
				const assetPath = join(dir, url);
				const origin = "https://raw.githubusercontent.com";
				return `${origin}/${owner}/${repo ?? DEFAULT_REPO}/${branch ?? "HEAD"}/${assetPath}`;
			}
			return url;
		},
		link: (url) => {
			if (repoData.owner) {
				const { repo, path, branch } = repoData;
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
			{isDev && SHOW_DEV_TOOLS && repoData !== null && (
				<RepoDevTools {...repoData} />
			)}

			<Post
				content={mdx}
				lastCommit={lastCommit}
				filename={repoData.path}
				config={config}
			/>
			<hr className="mb-4" />
			<Footer />
		</article>
	);
};

export default Page;
