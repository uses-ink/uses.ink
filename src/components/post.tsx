"use client";

import { runMDX } from "@/lib/mdx";
import { mdxComponents } from "@/lib/mdx/components";
import { useEffect } from "react";
import Moment from "react-moment";
import {
	Tooltip,
	TooltipContent,
	TooltipProvider,
	TooltipTrigger,
} from "./ui/tooltip";
import type { CommitResponse } from "@/lib/types";

export default function Post({
	filename,
	content,
	lastCommit,
}: {
	filename?: string;
	content: string;
	lastCommit: CommitResponse | null;
}) {
	const {
		meta: {
			title,
			description,
			author,
			date,
			hideTop,
			readingTime: enableReadingTime,
		},
		readingTime: { text: readTime, words },
		Content,
	} = runMDX(content);

	console.log("Post -> enableReadingTime", enableReadingTime);

	const resolvedDate = date ?? lastCommit?.date;
	const { resolvedAuthor, link, avatar } = lastCommit?.author
		? {
				resolvedAuthor: lastCommit.author.name,
				link: `https://github.com/${lastCommit.author.login}`,
				avatar: lastCommit.author.avatar,
			}
		: { resolvedAuthor: author, link: null, avatar: null };
	let resolvedTitle = title;
	if (!resolvedTitle) {
		const content =
			Content({ components: mdxComponents })?.props.children ?? [];
		// if is iterable
		if (Symbol.iterator in Object(content)) {
			for (const child of content) {
				if (["h1", "h2", "h3"].includes(child.type)) {
					resolvedTitle = child.props.children;
					break;
				}
			}
		} else {
			if (["h1", "h2", "h3"].includes(content.type)) {
				resolvedTitle = content.props;
			}
		}
		if (!resolvedTitle) {
			const lastPath = filename?.split("/").pop();
			const lastPathWithoutExtension = lastPath?.split(".").shift() ?? lastPath;
			if (lastPathWithoutExtension)
				resolvedTitle =
					lastPathWithoutExtension.slice(0, 1).toUpperCase() +
					lastPathWithoutExtension.slice(1);
		}
	}
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
	return (
		<>
			{!hideTop && (
				<header className="mb-8">
					<h1>{title}</h1>

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
								•
							</>
						)}{" "}
						<TooltipProvider>
							{resolvedDate && (
								<Tooltip>
									<TooltipTrigger>
										<a
											className="not-prose"
											href={lastCommit?.link}
											target="_blank"
											rel="noreferrer"
										>
											Last updated <Moment fromNow>{resolvedDate}</Moment> •
										</a>
									</TooltipTrigger>
									<TooltipContent>
										<Moment format="LLL">{resolvedDate}</Moment>
									</TooltipContent>
								</Tooltip>
							)}{" "}
							{enableReadingTime && (
								<Tooltip>
									<TooltipTrigger>
										<span>{readTime}</span>
									</TooltipTrigger>
									<TooltipContent>{words} words</TooltipContent>
								</Tooltip>
							)}
						</TooltipProvider>
					</p>
				</header>
			)}

			<Content components={mdxComponents} />
		</>
	);
}
