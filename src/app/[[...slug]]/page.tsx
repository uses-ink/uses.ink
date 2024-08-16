import Post from "@/components/client/post";
import Article from "@/components/server/article";
import { RepoDevTools } from "@/components/server/repo";
import { getRepoRequest } from "@/lib/server/repo-request";
import {
	DEFAULT_REF,
	DEFAULT_REPO,
	EXTENSIONS,
	IS_DEV,
	SHOW_DEV_TOOLS,
} from "@/lib/constants";
import { FetchError } from "@/lib/errors";
import {
	fetchConfig,
	fetchData,
	fetchLocalData,
	fetchUserConfig,
} from "@/lib/server/fetch";
import { compileMDX } from "@/lib/server/mdx";
import type { GitHubRequest } from "@/lib/types";
import type { NextPage } from "next";
import { dirname, join } from "node:path";
import { ZodError } from "zod";
import ErrorPage from "./error";
import AutoReadme from "@/components/server/auto-readme";
import { serverLogger } from "@/lib/server/logger";
import { resolveMetadata } from "@/lib/server/utils";

const Page: NextPage = async () => {
	const { req: repoRequest, url, host } = getRepoRequest();
	serverLogger.info({ repoRequest, url, host });

	const isRemote = !!repoRequest.owner;

	try {
		const userConfig = isRemote
			? await fetchUserConfig(repoRequest.owner as string)
			: undefined;
		const { content, lastCommit, fileName } = isRemote
			? await fetchData({
					...repoRequest,
					repo: repoRequest.repo ?? userConfig?.defaultRepo ?? DEFAULT_REPO,
					ref: repoRequest.ref ?? userConfig?.defaultBranch ?? DEFAULT_REF,
					path: repoRequest.path ?? "",
				} as GitHubRequest)
			: await fetchLocalData(repoRequest.path ?? "README.md");

		const extension = fileName.split(".").pop() ?? "md";

		if (EXTENSIONS.indexOf(extension) === -1) {
			return (
				<ErrorPage repoData={repoRequest} error="File type not supported" />
			);
		}
		const urlResolvers = {
			asset: (url: string) => {
				if (repoRequest.owner) {
					const { owner, repo, path, ref } = repoRequest;
					const dir = dirname(path ?? "");
					const assetPath = join(dir, url);
					return `https://raw.githubusercontent.com/${owner}/${repo ?? DEFAULT_REPO}/${ref ?? DEFAULT_REF}/${assetPath}`;
				}
				return url;
			},
			link: (url: string) => {
				if (repoRequest.owner) {
					const { repo, path, ref } = repoRequest;
					const dir = dirname(path ?? "");
					const assetPath = join(dir, url);
					return `/${repo ?? DEFAULT_REPO}${ref ? `@${ref}` : ""}/${assetPath}`;
				}

				return url;
			},
		};

		const repoConfig = isRemote
			? await fetchConfig({
					...repoRequest,
					repo: repoRequest.repo ?? DEFAULT_REPO,
					path: repoRequest.path ?? "",
				} as GitHubRequest)
			: undefined;

		const config = {
			...userConfig,
			...repoConfig,
		};

		const res = await compileMDX(content, urlResolvers, config);

		if (res instanceof ZodError) {
			return (
				<Article>
					<FrontmatterError errors={res.errors} />
				</Article>
			);
		}

		const resolvedMeta = resolveMetadata(
			res.meta,
			// res.runnable,
			repoRequest,
			lastCommit,
			fileName,
		);

		return (
			<>
				{/* This gets automatically added to the <head> */}
				<title>{resolvedMeta.title}</title>
				<meta name="description" content={resolvedMeta.description} />
				<meta property="og:title" content={resolvedMeta.title} />
				<meta property="og:description" content={resolvedMeta.description} />
				<meta property="og:type" content="website" />
				<meta property="og:url" content={`https://${host}${url.pathname}`} />
				{res.meta.image ? (
					<meta
						property="og:image"
						content={
							/^(https?:)?\/\//.test(res.meta.image)
								? res.meta.image
								: urlResolvers.asset(res.meta.image)
						}
					/>
				) : resolvedMeta.author.avatar ? (
					<meta property="og:image" content={resolvedMeta.author.avatar} />
				) : null}

				<Article>
					{IS_DEV && SHOW_DEV_TOOLS && <RepoDevTools {...repoRequest} />}

					<Post
						runnable={res.runnable}
						meta={res.meta}
						lastCommit={lastCommit}
						config={repoConfig}
						resolvedMeta={resolvedMeta}
					/>
				</Article>
			</>
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
		serverLogger.error(error);

		return <ErrorPage repoData={repoRequest} error="Unknown error" />;
	}
};

export default Page;
