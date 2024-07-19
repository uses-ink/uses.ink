import Post from "@/components/post";
import { DEFAULT_BRANCH, DEFAULT_REPO } from "@/lib/constants";
import nodepath from "node:path";
import { compileMDX } from "@/lib/mdx";
import { fetchPost } from "@/lib/post";
import { fetchReadme } from "@/lib/readme";
import type { GitHubRequest } from "@/lib/types";
import type { NextPage } from "next";
import { headers } from "next/headers";
import ThemeSelect from "@/components/theme-select";
import { Rss } from "lucide-react";

const isDev = process.env.NODE_ENV === "development";

function getRepo(): {
	owner: string;
	repo: string;
	branch: string;
	path?: string;
} | null {
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
		let path = url.split(/https?:\/\/[^/]+/)[1];
		if (path.includes("@")) {
			const [pathParts, branchPart] = path.split("@");
			path = pathParts;
			branch = branchPart;
		}
		console.log("path", path);
		const pathParts = path.split("/").filter(Boolean);
		repo = pathParts.shift();
		folder = pathParts.join("/");
	}

	// <username>.uses.ink/[repo]/[folder]@[branch]
	// cestef.uses.ink/notes@main
	// cestef.uses.ink/notes/2021-09-01
	// cestef.uses.ink/notes/2021-09-01@master

	if (owner) {
		return {
			owner: owner,
			repo: repo ?? DEFAULT_REPO,
			branch: branch ?? DEFAULT_BRANCH,
			path: folder,
		};
	}

	return null;
}

const Page: NextPage = async () => {
	const repoData = getRepo();

	if (!repoData) {
		return <div>Invalid repo</div>;
	}

	const request: GitHubRequest = {
		owner: repoData.owner,
		repo: repoData.repo,
		path: repoData.path ?? "",
	};

	const content = await (["mdx", "md"].includes(
		request.path.split(".").pop() ?? "",
	)
		? fetchPost
		: fetchReadme)(request);

	const mdx = await compileMDX(content, {
		asset: (url) => {
			const { owner, repo, path } = request;
			const dirname = nodepath.dirname(path);
			const assetPath = nodepath.join(dirname, url);
			const origin = "https://raw.githubusercontent.com";
			return `${origin}/${owner}/${repo}/HEAD/${assetPath}`;
		},
		link: (url) => {
			const { repo, path } = request;
			const dirname = nodepath.dirname(path);
			const assetPath = nodepath.join(dirname, url);
			return `/${repo}/${assetPath}`;
		},
	});

	return (
		<div className="container mx-auto">
			{isDev && (
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
			<div>
				<Post content={mdx} />
				<div className="flex justify-center mb-12">
					<div className="flex gap-4 items-center justify-between">
						<ThemeSelect />
						<span className="cursor-pointer flex">
							<Rss className="w-5 h-5 mr-2" />
							RSS
						</span>
					</div>
				</div>
			</div>
		</div>
	);
};

export default Page;
