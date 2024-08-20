import { DEFAULT_REPO, IS_DEV, SHOW_DEV_TOOLS } from "@uses.ink/constants";
import { RepoDevTools } from "./repo";
import type { GithubTree, RepoRequest } from "@uses.ink/types";

import Article from "./article";
import React from "react";

const Readme = ({
	repoRequest,
	filteredTree,
	path,
}: {
	repoRequest: RepoRequest;
	filteredTree: GithubTree["tree"];
	path: string;
}) => (
	<Article auto>
		{IS_DEV && SHOW_DEV_TOOLS && repoRequest !== null && (
			<RepoDevTools {...repoRequest} />
		)}
		<h1 className="text-xl font-bold flex flex-wrap gap-y-4 gap-x-1">
			<a
				href={`https://github.com/${repoRequest.owner}`}
				target="_blank"
				rel="noopener noreferrer"
				className="no-underline font-bold hover:underline"
			>
				<img
					src={`https://github.com/${repoRequest.owner}.png`}
					alt={`${repoRequest.owner}'s avatar`}
					className="w-12 h-12 rounded-full inline-block mr-4 align-middle border border-muted-foreground"
				/>
			</a>
			<a href={"/"} className="no-underline font-bold hover:underline">
				{repoRequest.owner}
			</a>
			<span className="text-muted-foreground mx-1">/</span>
			<a
				href={`/${repoRequest.repo ?? DEFAULT_REPO}${
					repoRequest.ref ? `@${repoRequest.ref}` : ""
				}`}
				className="no-underline font-bold hover:underline"
			>
				{repoRequest.repo}
			</a>
			{repoRequest.path?.split("/").map((part, index, parts) => (
				<React.Fragment key={part}>
					<span className="text-muted-foreground mx-1">/</span>
					{index === parts.length - 1 ? (
						<span className="font-bold">{part}</span>
					) : (
						<a
							href={`/${repoRequest.repo ?? DEFAULT_REPO}${
								repoRequest.ref ? `@${repoRequest.ref}` : ""
							}/${parts.slice(0, index + 1).join("/")}`}
							className="no-underline font-bold hover:underline"
						>
							{part}
						</a>
					)}
				</React.Fragment>
			))}
		</h1>
		<ul className="list-disc pl-6">
			{filteredTree.length > 0 ? (
				filteredTree.map((file) => (
					<h2 key={file.path}>
						<li>
							<a
								href={`/${repoRequest.repo ?? DEFAULT_REPO}${
									repoRequest.ref ? `@${repoRequest.ref}` : ""
								}/${file.path}`}
							>
								{file.path?.replace(`${path}/`, "")}
							</a>
						</li>
					</h2>
				))
			) : (
				<li>No markdown files found in this directory</li>
			)}
		</ul>
	</Article>
);

export default Readme;
