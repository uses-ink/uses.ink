"use client";

import { runMDX } from "@/lib/mdx";
import { mdxComponents } from "@/lib/mdx/components";
import { useEffect } from "react";
import Moment from "react-moment";

export default function Post({
	content,
	lastCommit,
}: {
	content: string;
	lastCommit: {
		date: string;
		author: { name: string; login: string; avatar: string };
	} | null;
}) {
	const {
		meta: { title, description, author, date, hideTop },
		readingTime: { text: readTime },
		Content,
	} = runMDX(content);
	useEffect(() => {
		if (title) {
			document.title = title;
		}
		if (description) {
			document
				.querySelector('meta[name="description"]')
				?.setAttribute("content", description);
		}
	}, [title, description]);
	const resolvedDate = date ?? lastCommit?.date;
	const { resolvedAuthor, link } = lastCommit?.author
		? {
				resolvedAuthor: lastCommit.author.name,
				link: `https://github.com/${lastCommit.author.login}`,
			}
		: { resolvedAuthor: author, link: null };
	return (
		<>
			<article className="prose md:prose-lg dark:prose-invert">
				{!hideTop && (
					<header className="mb-8">
						<h1 className="text-3xl font-bold">{title}</h1>
						<p className="text-sm text-gray-500">
							{resolvedAuthor && (
								<>
									By{" "}
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
							{resolvedDate && (
								<>
									Last updated <Moment fromNow>{resolvedDate}</Moment> •
								</>
							)}{" "}
							{readTime}
						</p>
					</header>
				)}
				<Content components={mdxComponents} />
			</article>
		</>
	);
}
