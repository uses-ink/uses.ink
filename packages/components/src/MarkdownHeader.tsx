import {
	TooltipProvider,
	Tooltip,
	TooltipTrigger,
	TooltipContent,
} from "./ui/tooltip";
import type { HeadingMeta, Meta, ReadingTime } from "@uses.ink/types";
import moment from "moment";
import React from "react";

export const MarkdownHeader = ({
	meta,
	headingMeta,
	readingTime,
}: { meta: Meta; headingMeta: HeadingMeta; readingTime?: ReadingTime }) => {
	const Author = headingMeta.author && (
		<>
			{headingMeta.author.avatar && (
				<span>
					<img
						src={headingMeta.author.avatar}
						alt={headingMeta.author.name}
						className="w-6 h-6 rounded-full inline-block mr-2"
					/>
				</span>
			)}
			<b>
				{headingMeta.author.link ? (
					<a href={headingMeta.author.link} target="_blank" rel="noreferrer">
						{headingMeta.author.name}
					</a>
				) : (
					headingMeta.author.name
				)}{" "}
			</b>
		</>
	);
	const PostDate = headingMeta.date && (
		<Tooltip>
			<TooltipTrigger>
				<a
					className="not-prose"
					href={headingMeta?.dateLink ?? undefined}
					target="_blank"
					rel="noreferrer"
				>
					Last updated {moment(headingMeta?.date).fromNow()}
				</a>
			</TooltipTrigger>
			<TooltipContent className="!text-[--tw-prose-body]">
				{moment(headingMeta?.date).format("LLL")}
			</TooltipContent>
		</Tooltip>
	);
	const ReadingTime = meta.readingTime && readingTime && (
		<Tooltip>
			<TooltipTrigger>
				<span>{readingTime.text}</span>
			</TooltipTrigger>
			<TooltipContent className="!text-[--tw-prose-body]">
				{readingTime.words} words
			</TooltipContent>
		</Tooltip>
	);
	const elements = [Author, PostDate, ReadingTime].filter(Boolean);
	return (
		!meta.hideTop && (
			<header className="mb-8">
				<h1>{meta.title}</h1>

				<div className="text-sm">
					<TooltipProvider>
						{/* join elements with " • " */}
						{elements.map((element, index) => (
							<React.Fragment key={index.toString()}>
								{index > 0 && (
									<span className="text-muted-foreground"> • </span>
								)}
								{element}
							</React.Fragment>
						))}
					</TooltipProvider>
				</div>
			</header>
		)
	);
};
