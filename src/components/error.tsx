import type { getRepo } from "@/lib/repo";

export const ErrorPage = ({
	repoData,
	error,
}: {
	repoData: ReturnType<typeof getRepo>;
	error: string;
}) => {
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
};
