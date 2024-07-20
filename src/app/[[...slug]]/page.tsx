import Post from "@/components/post";
import { SHOW_DEV_TOOLS } from "@/lib/constants";
import nodepath, { join } from "node:path";
import { compileMDX } from "@/lib/mdx";
import { fetchPost } from "@/lib/post";
import { fetchReadme } from "@/lib/readme";
import type { GitHubRequest } from "@/lib/types";
import type { NextPage } from "next";
import { headers } from "next/headers";
import ThemeSelect from "@/components/theme-select";
import { Rss } from "lucide-react";
import { readingTime } from "reading-time-estimator";
import { isErrorHasStatus } from "@/lib/github";
import { promises as fs } from "node:fs";
const isDev = process.env.NODE_ENV === "development";

function getRepo() {
	const headersList = headers();
	const host = headersList.get("host");
	const url = headersList.get("x-url");
	console.log("host", host);
	console.log("url", url);
	const isLocalhost = host?.includes("localhost");
	const parts = host?.split(".") ?? [];
	let owner = null;
	if (isLocalhost) {
		if (parts.length > 1) {
			owner = parts[0];
		}
	} else if (parts.length > 2) {
		owner = parts[0];
	}
	let repo = undefined;
	let branch = undefined;
	let folder = undefined;
	if (url) {
		const path = url.split(/https?:\/\/[^/]+/)[1];
		console.log("path", path);
		const pathParts = path.split("/").filter(Boolean);
		repo = pathParts.shift();

		if (repo?.includes("@")) {
			const [repoPart, branchPart] = path.split("@");
			repo = repoPart;
			branch = branchPart;
		}
		folder = pathParts.join("/");
	}

	// <username>.uses.ink/[repo]@[branch]/[folder]
	// cestef.uses.ink/notes@main
	// cestef.uses.ink/notes/2021-09-01
	// cestef.uses.ink/notes@master/2021-09-01

	return {
		owner: owner,
		repo: repo,
		branch: branch,
		path: folder || undefined,
	};
}

const fetchData = async (
	request: GitHubRequest,
): Promise<
	| {
			content: string;
			lastCommit: {
				date: string;
				author: {
					name: string;
					login: string;
					avatar: string;
				};
			} | null;
			error: undefined;
	  }
	| {
			content: null;
			lastCommit: null;
			error: string;
	  }
> => {
	try {
		const { content, lastCommit } = await (["mdx", "md"].includes(
			request.path.split(".").pop() ?? "",
		)
			? fetchPost
			: fetchReadme)(request);

		return { content, lastCommit, error: undefined };
	} catch (
		// biome-ignore lint/suspicious/noExplicitAny: <explanation>
		error: any
	) {
		console.error("Error fetching data", error);
		if (isErrorHasStatus(error)) {
			switch (error.status) {
				case 404:
					return { content: null, lastCommit: null, error: "Not found" };
				default:
					return {
						content: null,
						lastCommit: null,
						error: `Error status ${error.status}`,
					};
			}
		}
		return { content: null, lastCommit: null, error: error.toString() };
	}
};

const fetchLocalData = async (path: string) => {
	console.log("path", JSON.stringify(path));
	const toRead = join(process.cwd(), "blog", path);
	console.log("toRead", toRead);
	const content = await fs.readFile(toRead, "utf-8");
	return { content, lastCommit: null, error: undefined };
};

const Page: NextPage = async () => {
	const repoData = getRepo();

	const { content, lastCommit, error } =
		repoData.owner !== null && repoData.repo !== null
			? await fetchData({
					...repoData,
					path: repoData.path ?? "",
				} as GitHubRequest)
			: await fetchLocalData(repoData.path ?? "README.md");

	if (error !== undefined) {
		return (
			<div className="flex w-screen h-screen justify-center items-center">
				<div className="text-center flex gap-2 flex-col">
					<h1 className="text-4xl">
						An error occured: <b>{error}</b>
					</h1>
					<p className="text-2xl dark:text-gray-400 text-gray-600">
						<a
							href={`https://github.com/${repoData.owner}/${repoData.repo}/tree/${repoData.branch}/${repoData.path}`}
							target="_blank"
							rel="noreferrer"
						>
							Check the repository and path provided in the URL.
						</a>
					</p>
				</div>
			</div>
		);
	}
	const mdx = await compileMDX(content, {
		asset: (url) => {
			if (repoData.owner && repoData.repo) {
				const { owner, repo, path } = repoData;
				const dirname = nodepath.dirname(path ?? "");
				const assetPath = nodepath.join(dirname, url);
				const origin = "https://raw.githubusercontent.com";
				return `${origin}/${owner}/${repo}/HEAD/${assetPath}`;
			}
			return url;
		},
		link: (url) => {
			if (repoData.owner && repoData.repo) {
				const { repo, path } = repoData;
				const dirname = nodepath.dirname(path ?? "");
				const assetPath = nodepath.join(dirname, url);
				return `/${repo}/${assetPath}`;
			}

			return url;
		},
	});

	return (
		<article
			className="container mx-auto prose max-md:prose-sm dark:prose-invert"
			dir="ltr"
		>
			{isDev && SHOW_DEV_TOOLS && repoData !== null && (
				<div className="absolute top-4 right-4 p-3 bg-red-500 text-white rounded-md">
					<h1>
						Repo:{" "}
						<a
							href={`https://github.com/${repoData.owner}/${repoData.repo}`}
							rel="noreferrer"
							target="_blank"
						>
							<b>
								{repoData.owner}/{repoData.repo}
							</b>
						</a>
					</h1>
					<h1>
						Branch: <b>{repoData.branch}</b>
					</h1>
					<h1>
						Path: <b>{repoData.path}</b>
					</h1>
				</div>
			)}

			<Post content={mdx} lastCommit={lastCommit} />
			{/* <div className="flex justify-center mb-12">
				<div className="flex gap-4 items-center justify-between">
					<ThemeSelect />
					<span className="cursor-pointer flex">
						<Rss className="w-5 h-5 mr-2" />
						RSS
					</span>
				</div>
			</div> */}
		</article>
	);
};

export default Page;
