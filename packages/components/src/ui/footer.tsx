import { useEffect, useState } from "react";
import {
	Tooltip,
	TooltipContent,
	TooltipProvider,
	TooltipTrigger,
} from "./tooltip";
import { cn } from "../lib/utils";

export const Footer = ({
	version,
	commit,
	time,
}: {
	version: string;
	commit: string;
	time: string;
}) => {
	const [isAltDown, setIsAltDown] = useState(false);

	useEffect(() => {
		const handleKeyDown = (e: KeyboardEvent) => {
			if (e.key === "Alt") {
				setIsAltDown(true);
			}
		};

		const handleKeyUp = (e: KeyboardEvent) => {
			if (e.key === "Alt") {
				setIsAltDown(false);
			}
		};

		document.addEventListener("keydown", handleKeyDown);
		document.addEventListener("keyup", handleKeyUp);

		return () => {
			document.removeEventListener("keydown", handleKeyDown);
			document.removeEventListener("keyup", handleKeyUp);
		};
	}, []);

	return (
		<TooltipProvider>
			<Tooltip>
				<TooltipTrigger>
					<p className="whitespace-nowrap !text-muted-foreground">
						Made with{" "}
						<a href="https://uses.ink" target="_blank" rel="noreferrer">
							uses.ink
						</a>
					</p>
				</TooltipTrigger>
				<TooltipContent className="border-border">
					<a
						href={
							version?.includes(commit?.slice(0, 7) ?? "")
								? `https://github.com/uses-ink/uses.ink/commit/${commit?.slice(0, 7)}`
								: `https://github.com/uses-ink/uses.ink/releases/tag/${version}`
						}
						target="_blank"
						rel="noreferrer"
						className="not-prose"
					>
						{version ?? "v0.0.0"}{" "}
						<span className="dark:text-gray-500 text-gray-300">
							({commit?.slice(0, 7) ?? "00000000"})
						</span>
						<span
							className={cn("text-gray-400 transition-all duration-500", {
								"opacity-0 hidden": !isAltDown,
							})}
						>
							{" "}
							- {new Date(time).toLocaleString()}
						</span>
					</a>
				</TooltipContent>
			</Tooltip>
		</TooltipProvider>
	);
};
