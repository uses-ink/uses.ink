"use client";

import { getLayout } from "@/components/layouts";
import { Navbar } from "@/components/server/navbar";
import {
	Tooltip,
	TooltipContent,
	TooltipProvider,
	TooltipTrigger,
} from "@/components/ui/tooltip";
import { runMDX } from "@/lib/client/mdx/run";
import type { RepoRequest } from "@/lib/client/repo-request";
import { cn } from "@/lib/client/utils";
import type { CommitResponse, ConfigSchema, MetaSchema } from "@/lib/types";
import { ChevronUp } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import Moment from "react-moment";
import type { z } from "zod";
import SearchableContent from "./search";
import {
	capitalizeFileName,
	resolveTitle,
	userContentHash,
} from "@/lib/client/utils";
import Head from "next/head";

export default function Post({
	filename,
	runnable,
	lastCommit,
	config,
	meta,
	request,
	url,
}: {
	filename?: string;
	runnable: string;
	lastCommit: CommitResponse | null;
	config: z.infer<typeof ConfigSchema> | null;
	meta: z.infer<typeof MetaSchema>;
	request: RepoRequest;
	url: string;
}) {
	const [canScroll, setCanScroll] = useState(false);

	const { readingTime, Content } = runMDX(runnable);

	useEffect(() => {
		const handleScroll = () => {
			setCanScroll(window.scrollY > 100);
		};

		const { handleClick, handleHashChange } = userContentHash();

		window.addEventListener("scroll", handleScroll);
		window.addEventListener("hashchange", handleHashChange);
		document.addEventListener("click", handleClick, false);
		return () => {
			window.removeEventListener("scroll", handleScroll);
			window.removeEventListener("hashchange", handleHashChange);
			document.removeEventListener("click", handleClick);
		};
	}, []);

	const scrollUp = useCallback(() => {
		window.scrollTo({ top: 0, behavior: "smooth" });
	}, []);

	useEffect(() => {
		document
			.querySelector('meta[name="description"]')
			?.setAttribute("content", meta.description ?? "");

		if (Content) {
			document.title =
				meta.title ??
				resolveTitle(Content) ??
				(filename
					? capitalizeFileName(filename)
					: `${request.owner}/${request.repo}`);
		}
	}, [
		Content,
		meta.title,
		meta.description,
		filename,
		request.owner,
		request.repo,
	]);

	const { layout } = meta;

	const Layout = getLayout(layout, Content);

	const resolvedDate = meta.date ?? lastCommit?.date;

	const { resolvedAuthor, link, avatar } = lastCommit?.author
		? {
				resolvedAuthor: lastCommit.author.name,
				link: `https://github.com/${lastCommit.author.login}`,
				avatar: lastCommit.author.avatar,
			}
		: { resolvedAuthor: meta.author, link: null, avatar: null };

	return (
		<>
			<Head>
				<meta name="description" content={meta.description ?? ""} />
				<meta property="og:title" content={meta.title} />
				<meta property="og:description" content={meta.description ?? ""} />
				<meta property="og:type" content="article" />
				<meta property="og:url" content={url} />
				<meta property="og:image" content={meta.image} />
				<meta property="og:site_name" content={"uses.ink"} />
			</Head>
			{meta.nav && <Navbar routes={meta.nav} />}
			{!(config?.hideTop ?? meta.hideTop) && (
				<header className="mb-8">
					<h1>{meta.title}</h1>

					<p className="text-sm text-gray-500">
						{resolvedAuthor && (
							<>
								{avatar && (
									<span>
										<img
											src={avatar}
											alt={resolvedAuthor}
											className="w-6 h-6 rounded-full inline-block mr-2"
										/>
									</span>
								)}
								<b>
									{link ? (
										<a href={link} target="_blank" rel="noreferrer">
											{resolvedAuthor}
										</a>
									) : (
										resolvedAuthor
									)}{" "}
								</b>
							</>
						)}
						{resolvedAuthor && resolvedDate && " • "}
						<TooltipProvider>
							{resolvedDate && (
								<Tooltip>
									<TooltipTrigger>
										<a
											className="not-prose"
											href={lastCommit?.link ?? undefined}
											target="_blank"
											rel="noreferrer"
											// The timezone of the server may be different from the user's timezone
											suppressHydrationWarning
										>
											Last updated <Moment fromNow>{resolvedDate}</Moment>
										</a>
									</TooltipTrigger>
									<TooltipContent
										// The timezone of the server may be different from the user's timezone
										suppressHydrationWarning
									>
										<Moment format="LLL">{resolvedDate}</Moment>
									</TooltipContent>
								</Tooltip>
							)}
							{resolvedDate &&
								(config?.readingTime ?? meta.readingTime) &&
								" • "}
							{(config?.readingTime ?? meta.readingTime) && (
								<Tooltip>
									<TooltipTrigger>
										<span>{readingTime.text}</span>
									</TooltipTrigger>
									<TooltipContent>{readingTime.words} words</TooltipContent>
								</Tooltip>
							)}
						</TooltipProvider>
					</p>
				</header>
			)}

			<SearchableContent ContentComponent={Layout} contentProps={{}} />

			<div className="fixed sm:bottom-8 sm:right-8 bottom-4 right-4">
				<ChevronUp
					className={cn(
						"h-4 w-4 sm:w-6 sm:h-6 cursor-pointer transition-opacity ease-in-out duration-300",
						{
							"opacity-100": canScroll,
							"opacity-0": !canScroll,
						},
					)}
					onClick={scrollUp}
				/>
			</div>
		</>
	);
}
