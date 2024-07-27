"use client";

import { GalleryImage, mdxComponents } from "@/lib/mdx/components";
import type { CommitResponse, ConfigSchema } from "@/lib/types";
import { cn } from "@/lib/utils";
import { ChevronUp } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import Moment from "react-moment";
import type { z } from "zod";
import { Loading } from "../loading";
import {
	Tooltip,
	TooltipContent,
	TooltipProvider,
	TooltipTrigger,
} from "../ui/tooltip";
import { resolveTitle, useRunMDX } from "./utils";
import SearchableContent from "../search";
import { Navbar } from "../navbar";

export default function Post({
	filename,
	content,
	lastCommit,
	config,
}: {
	filename?: string;
	content: string;
	lastCommit: CommitResponse | null;
	config: z.infer<typeof ConfigSchema> | null;
}) {
	const [canScroll, setCanScroll] = useState(false);
	const [resolvedTitle, setResolvedTitle] = useState<string | null>(null);
	const [description, setDescription] = useState<string | null>(null);
	const [searchText, setSearchText] = useState("");

	useEffect(() => {
		const handleScroll = () => {
			setCanScroll(window.scrollY > 100);
		};
		window.addEventListener("scroll", handleScroll);
		return () => {
			window.removeEventListener("scroll", handleScroll);
		};
	}, []);

	const scrollUp = useCallback(() => {
		window.scrollTo({ top: 0, behavior: "smooth" });
	}, []);

	useEffect(() => {
		if (resolvedTitle) {
			document.title = resolvedTitle;
		}
		if (description) {
			document
				.querySelector('meta[name="description"]')
				?.setAttribute("content", description);
		}
	}, [resolvedTitle, description]);

	const { meta, readingTime, Content } = useRunMDX(content);

	useEffect(() => {
		if (meta?.success) {
			setDescription(meta.data.description ?? null);
			if (Content) {
				setResolvedTitle(meta.data.title ?? resolveTitle(Content));
			}
		}
	}, [meta, Content]);

	if (!meta) {
		return <Loading />;
	}

	if (!meta.success) {
		return (
			<div className="flex w-screen justify-center items-center prose max-w-full dark:prose-invert">
				<div className="text-center flex gap-2 flex-col items-center">
					<h1 className="text-4xl">
						An error occured when parsing your frontmatter.
					</h1>
					<pre className="text-left language-json">{meta.error.message}</pre>
					<p className="text-lg dark:text-gray-400 text-gray-600">
						See the{" "}
						<a
							href="https://uses.ink/docs/frontmatter"
							target="_blank"
							rel="noreferrer"
						>
							frontmatter documentation
						</a>{" "}
						for more information.
					</p>
					<h3>
						<a href="https://uses.ink">Back to uses.ink</a>
					</h3>
				</div>
			</div>
		);
	}
	const { layout } = meta.data;
	const getLayout = () => {
		switch (layout) {
			case "gallery":
				return GalleryLayout;
			default:
				return PostLayout;
		}
	};

	const Layout = getLayout();
	const LayoutContent = () => (
		<Layout>
			<Content
				components={{
					...mdxComponents,
					...(layout === "gallery" ? { img: GalleryImage as any } : {}),
				}}
			/>
		</Layout>
	);

	const resolvedDate = meta.data.date ?? lastCommit?.date;

	const { resolvedAuthor, link, avatar } = lastCommit?.author
		? {
				resolvedAuthor: lastCommit.author.name,
				link: `https://github.com/${lastCommit.author.login}`,
				avatar: lastCommit.author.avatar,
			}
		: { resolvedAuthor: meta.data.author, link: null, avatar: null };

	return (
		<>
			{meta.data.nav && <Navbar routes={meta.data.nav} />}
			{!(config?.hideTop ?? meta.data.hideTop) && (
				<header className="mb-8">
					<h1>{meta.data.title}</h1>

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
										>
											Last updated <Moment fromNow>{resolvedDate}</Moment>
										</a>
									</TooltipTrigger>
									<TooltipContent>
										<Moment format="LLL">{resolvedDate}</Moment>
									</TooltipContent>
								</Tooltip>
							)}
							{resolvedDate &&
								(config?.readingTime ?? meta.data.readingTime) &&
								" • "}
							{(config?.readingTime ?? meta.data.readingTime) && (
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

			<SearchableContent ContentComponent={LayoutContent} contentProps={{}} />

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

function PostLayout({ children }: { children: React.ReactNode }) {
	return <>{children}</>;
}

function GalleryLayout({ children }: { children: React.ReactNode }) {
	return (
		// spread the children to the grid horizontally
		<div className="grid grid-cols-1 md:grid-cols-2 gap-4">{children}</div>
	);
}
