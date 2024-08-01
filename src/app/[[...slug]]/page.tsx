import Post from "@/components/client/post";
import Article from "@/components/server/article";
import Readme from "@/components/server/readme";
import { RepoDevTools } from "@/components/server/repo";
import { getRepoRequest } from "@/lib/server/repo-request";
import {
	DEFAULT_REPO,
	EXTENSIONS,
	IS_DEV,
	SHOW_DEV_TOOLS,
} from "@/lib/constants";
import { FetchError } from "@/lib/errors";
import { fetchConfig, fetchData, fetchLocalData } from "@/lib/server/fetch";
import { fetchGithubTree } from "@/lib/server/github/tree";
import { compileMDX } from "@/lib/server/mdx";
import type { GitHubRequest } from "@/lib/types";
import type { NextPage } from "next";
import { dirname, join } from "node:path";
import { ZodError } from "zod";
import ErrorPage from "./error";
import AutoReadme from "@/components/server/auto-readme";
import { serverLogger } from "@/lib/server/logger";

const Page: NextPage = async () => {
	const { req: repoRequest, url } = getRepoRequest();
	serverLogger.info({ repoRequest, url });

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

		serverLogger.debug({ config });

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
			<Article>
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
						url={url}
					/>
				)}
			</Article>
		);
	} catch (error) {
		if (error instanceof FetchError) {
			const isReadmeRequest =
				!repoRequest.path ||
				!EXTENSIONS.includes(repoRequest.path.split(".").pop() ?? "");
			serverLogger.debug({ isReadmeRequest });
			if (error.name === "NOT_FOUND" && isReadmeRequest && isRemote) {
				// Generate a readme from the current directory
				return <AutoReadme repoRequest={repoRequest} />;
			}
			serverLogger.error({ error, where: "Page" });
			return <ErrorPage repoData={repoRequest} error={error.message} />;
		}
		serverLogger.error({ error, where: "Page" });

		return <ErrorPage repoData={repoRequest} error="Unknown error" />;
	}
};

export default Page;
