import MarkdownPost from "@/components/client/md-post";
import Article from "@/components/server/article";
import { RepoDevTools } from "@/components/server/repo";
import {
	fetchConfig,
	fetchData,
	fetchLocalData,
	fetchUserConfig,
} from "@/lib/server/fetch";
import { getRepoRequest } from "@/lib/server/repo-request";
import {
	DEFAULT_REF,
	DEFAULT_REPO,
	EXTENSIONS,
	IS_DEV,
	SHOW_DEV_TOOLS,
} from "@uses.ink/constants";
import { FetchError } from "@uses.ink/errors";
import { compileMDX, compileTypst } from "@uses.ink/render";
import {
	FileType,
	fileTypeFromExtension,
	type GithubTree,
	type GitHubRequest,
} from "@uses.ink/types";
import type { NextPage } from "next";
import { dirname, join } from "node:path";
import { ZodError } from "zod";
import AutoReadme from "@/components/server/auto-readme";
import { fetchGithubTree } from "@/lib/server/github/tree";
import {
	isReadmeRequest,
	resolveMetadata,
	validateRequestAgainstTree,
} from "@/lib/server/utils";
import { serverLogger } from "@uses.ink/logger";
import ErrorPage from "./error";

const Page: NextPage = async () => {
	const { req: repoRequest, url, host } = getRepoRequest();
	serverLogger.info({
		repoRequest: Object.fromEntries(
			Object.entries(repoRequest).filter(([_, v]) => v),
		),
		url: url.toString(),
		host,
	});

	const isRemote = !!repoRequest.owner;
	let tree: GithubTree | undefined = undefined;
	try {
		const userConfig = isRemote
			? await fetchUserConfig(repoRequest.owner as string)
			: undefined;
		const githubRequest = {
			...repoRequest,
			repo: repoRequest.repo ?? userConfig?.defaultRepo ?? DEFAULT_REPO,
			ref: repoRequest.ref ?? userConfig?.defaultBranch ?? DEFAULT_REF,
			path: repoRequest.path ?? "",
		} as GitHubRequest;
		tree = isRemote ? await fetchGithubTree(githubRequest) : undefined;
		if (isRemote) {
			// biome-ignore lint/style/noNonNullAssertion: This is a valid check
			const validatedPath = validateRequestAgainstTree(githubRequest, tree!);
			serverLogger.debug({ validated: validatedPath });
			githubRequest.path = validatedPath;
		}
		const { content, lastCommit, fileName } = isRemote
			? await fetchData(
					githubRequest,
					// biome-ignore lint/style/noNonNullAssertion: This is a valid check
					tree!,
				)
			: await fetchLocalData(repoRequest.path ?? "README.md");

		const extension = fileName.split(".").pop() ?? "md";
		const fileType = fileTypeFromExtension(extension);
		if (EXTENSIONS.indexOf(extension) === -1 || fileType === null) {
			return (
				<ErrorPage
					repoData={repoRequest}
					error={`Unsupported file type: ${extension}`}
				/>
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

		let renderedContent = {} as { Head: React.FC; Post: React.FC };
		switch (fileType) {
			case FileType.Markdown: {
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

				const Head = () => (
					<>
						<title>{resolvedMeta.title}</title>
						<meta name="description" content={resolvedMeta.description} />
						<meta property="og:title" content={resolvedMeta.title} />
						<meta
							property="og:description"
							content={resolvedMeta.description}
						/>
						<meta property="og:type" content="website" />
						<meta
							property="og:url"
							content={`https://${host}${url.pathname}`}
						/>
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
					</>
				);

				const Post = () => (
					<MarkdownPost
						runnable={res.runnable}
						meta={res.meta}
						lastCommit={lastCommit}
						config={repoConfig}
						resolvedMeta={resolvedMeta}
					/>
				);

				renderedContent = { Head, Post };

				break;
			}
			case FileType.Typst: {
				const svg = await compileTypst(content, config);
				if (svg === null) {
					return (
						<ErrorPage
							repoData={repoRequest}
							error="Typst compilation failed"
						/>
					);
				}
				renderedContent = {
					Head: () => (
						<>
							<title>{fileName}</title>
							<meta name="description" content="Typst diagram" />
						</>
					),
					Post: () => (
						<div
							// biome-ignore lint/security/noDangerouslySetInnerHtml: Whatever
							dangerouslySetInnerHTML={{ __html: svg }}
							style={{ width: "100%", height: "100%" }}
						/>
					),
				};
				break;
			}
		}
		const { Head, Post } = renderedContent;
		return (
			<>
				{/* This gets automatically added to the <head> */}
				<Head />
				<Article>
					{IS_DEV && SHOW_DEV_TOOLS && <RepoDevTools {...repoRequest} />}
					<Post />
				</Article>
			</>
		);
	} catch (error) {
		if (error instanceof FetchError) {
			if (error.name === "NOT_FOUND" || error.name === "NOT_FOUND_PREFETCH") {
				if (isReadmeRequest(repoRequest) && isRemote)
					// Generate a readme from the current directory
					return <AutoReadme repoRequest={repoRequest} tree={tree} />;
				return <ErrorPage repoData={repoRequest} error="Not found" />;
			}
			serverLogger.error({ error, where: "Page" });
			return <ErrorPage repoData={repoRequest} error={error.message} />;
		}
		serverLogger.error(error);

		return <ErrorPage repoData={repoRequest} error="Unknown error" />;
	}
};

export default Page;
