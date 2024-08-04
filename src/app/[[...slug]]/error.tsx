"use client"; // Error components must be Client Components

import { clientLogger } from "@/lib/client/logger";
import type { RepoRequest } from "@/lib/server/repo-request";
import { useEffect } from "react";

const ErrorPage = ({
	repoData,
	error,
	reset,
}: {
	repoData: RepoRequest;
	error: string | (Error & { digest?: string });
	reset?: () => void;
}) => {
	useEffect(() => {
		clientLogger.error(error);
	}, [error]);

	// const is404 =
	// 	error.toLowerCase().includes("not found") ||
	// 	error.toLowerCase().includes("enoent");
	const is404 = ["Not found", "ENOENT"].includes(
		error instanceof Error ? error.message : error,
	);

	return (
		<div className="flex w-screen h-screen justify-center items-center prose max-w-full dark:prose-invert">
			<div className="text-center flex gap-2 flex-col items-center">
				<h1 className="text-4xl">
					An error occured:{" "}
					<b>{error instanceof Error ? error.message : error}</b>
				</h1>
				{is404 ? (
					<p className="text-2xl dark:text-gray-400 text-gray-600">
						{repoData?.owner ? (
							<a
								href={`https://github.com/${repoData.owner}/${repoData.repo}/tree/${repoData.ref}/${repoData.path}`}
								target="_blank"
								rel="noreferrer"
							>
								Check the repository and path provided in the URL.
							</a>
						) : (
							"Check the path provided in the URL."
						)}
					</p>
				) : (
					<button
						onClick={reset}
						type="button"
						className="rounded-md py-2 px-4 bg-gray-200 dark:bg-gray-800 w-fit"
					>
						Reload
					</button>
				)}
				<h3>
					<a href="https://uses.ink">Back to uses.ink</a>
				</h3>
			</div>
		</div>
	);
};

export default ErrorPage;
