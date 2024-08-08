import { DEFAULT_REPO, IS_DEV, SHOW_DEV_TOOLS } from "@/lib/constants";
import { RepoDevTools } from "./repo";
import type { RepoRequest } from "@/lib/types";
import type { GithubTree } from "@/lib/server/github/tree";
import Article from "./article";

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
		<h1 className="text-xl font-bold">
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
			{repoRequest.path && (
				<>
					<span className="text-muted-foreground mx-1">/</span>
					<span>{repoRequest.path}</span>
				</>
			)}
		</h1>
		<ul className="list-disc pl-6">
			{filteredTree.length > 0 ? (
				filteredTree.map((file) => (
					<li key={file.path}>
						<a
							href={`/${repoRequest.repo ?? DEFAULT_REPO}${
								repoRequest.ref ? `@${repoRequest.ref}` : ""
							}/${file.path}`}
						>
							<h2>{file.path?.replace(`${path}/`, "")}</h2>
						</a>
					</li>
				))
			) : (
				<li>No markdown files found in this directory</li>
			)}
		</ul>
	</Article>
);

export default Readme;
