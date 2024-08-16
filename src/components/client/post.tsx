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
import { cn, userContentHash } from "@/lib/client/utils";
import type { ResolvedMetadata } from "@/lib/server/utils";
import type { CommitResponse, ConfigSchema, MetaSchema } from "@/lib/types";
import { ChevronUp } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import Moment from "react-moment";
import type { z } from "zod";
import SearchableContent from "./search";

export default function Post({
	runnable,
	config,
	meta,
	resolvedMeta,
	lastCommit,
}: {
	runnable: string;
	config?: z.infer<typeof ConfigSchema>;
	meta: z.infer<typeof MetaSchema>;
	resolvedMeta: ResolvedMetadata;
	lastCommit?: CommitResponse;
}) {
	const [canScroll, setCanScroll] = useState(false);

	const { readingTime, Content } = runMDX(runnable);

	useEffect(() => {
		const handleScroll = () => {
			setCanScroll(window.scrollY > 100);
		};

		const { handleClick, handleHashChange } = userContentHash();

		// const onThemeChange = (theme: string) => {
		// 	flipImageSource(theme);
		// };

		// // Observe the html element for class changes
		// const observer = new MutationObserver((mutations) => {
		// 	for (const mutation of mutations) {
		// 		if (mutation.attributeName === "class") {
		// 			const theme = document.documentElement.classList.contains("dark")
		// 				? "dark"
		// 				: "light";
		// 			onThemeChange(theme);
		// 		}
		// 	}
		// });

		// observer.observe(document.documentElement, {
		// 	attributes: true,
		// });

		// // Check the current theme
		// const theme = document.documentElement.classList.contains("dark")
		// 	? "dark"
		// 	: "light";

		// onThemeChange(theme);

		window.addEventListener("scroll", handleScroll);
		window.addEventListener("hashchange", handleHashChange);
		document.addEventListener("click", handleClick, false);
		return () => {
			window.removeEventListener("scroll", handleScroll);
			window.removeEventListener("hashchange", handleHashChange);
			document.removeEventListener("click", handleClick);
			// observer.disconnect();
		};
	}, []);

	const scrollUp = useCallback(() => {
		window.scrollTo({ top: 0, behavior: "smooth" });
	}, []);

	const { layout } = meta;

	const Layout = getLayout(layout ?? "post", Content);

	return (
		<>
			{meta.nav && <Navbar routes={meta.nav} />}
			{!(config?.hideTop ?? meta.hideTop) && (
				<header className="mb-8">
					<h1>{meta.title}</h1>

					<p className="text-sm text-gray-500">
						{resolvedMeta.author && (
							<>
								{resolvedMeta.author.avatar && (
									<span>
										<img
											src={resolvedMeta.author.avatar}
											alt={resolvedMeta.author.name}
											className="w-6 h-6 rounded-full inline-block mr-2"
										/>
									</span>
								)}
								<b>
									{resolvedMeta.author.link ? (
										<a
											href={resolvedMeta.author.link}
											target="_blank"
											rel="noreferrer"
										>
											{resolvedMeta.author.name}
										</a>
									) : (
										resolvedMeta.author.name
									)}{" "}
								</b>
							</>
						)}
						{resolvedMeta.author.name && resolvedMeta.date && " • "}
						<TooltipProvider>
							{resolvedMeta.date && (
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
											Last updated <Moment fromNow>{resolvedMeta.date}</Moment>
										</a>
									</TooltipTrigger>
									<TooltipContent
										// The timezone of the server may be different from the user's timezone
										suppressHydrationWarning
									>
										<Moment format="LLL">{resolvedMeta.date}</Moment>
									</TooltipContent>
								</Tooltip>
							)}
							{resolvedMeta.date &&
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

			{/* <SearchableContent ContentComponent={Layout} contentProps={{}} /> */}
			<Layout />

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
