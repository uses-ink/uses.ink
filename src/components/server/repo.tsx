import type { RepoRequest } from "@/lib/server/repo-request";

export const RepoDevTools = (repoData: RepoRequest) => {
	return (
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
	);
};
