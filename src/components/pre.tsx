// Based on https://github.com/facebook/docusaurus/blob/ed9d2a26f5a7b8096804ae1b3a4fffc504f8f90d/packages/docusaurus-theme-classic/src/theme/CodeBlock/index.tsx
// which is under MIT License as per the banner

import { cn } from "@/lib/utils";
import { Check, Clipboard, WrapText } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { icons } from "./icons";

const Pre = ({
	children,
	...props
}: {
	children: React.ReactNode;
}) => {
	const preRef = useRef<HTMLPreElement>(null);
	const [showCopied, setShowCopied] = useState(false);

	const handleCopyCode = async () => {
		if (preRef.current) {
			const content = Array.from(
				preRef.current.querySelectorAll("code span.line"),
			)
				.map((el) => el.textContent)
				.join("\n");

			await navigator.clipboard.writeText(content);
		} else {
			console.error("pre.current is null");
			return;
		}
		setShowCopied(true);
		setTimeout(() => setShowCopied(false), 2000);
	};

	const toggleWordWrap = useCallback(() => {
		const htmlDataset = document.documentElement.dataset;
		const hasWordWrap = "wordWrap" in htmlDataset;
		if (hasWordWrap) {
			// biome-ignore lint/performance/noDelete: need to delete
			delete htmlDataset.wordWrap;
		} else {
			htmlDataset.wordWrap = "";
		}
	}, []);

	const [hasTitle, setHasTitle] = useState(false);
	const divRef = useRef<HTMLDivElement>(null);

	useEffect(() => {
		const element = divRef.current?.parentElement?.querySelector(
			"[data-rehype-pretty-code-title]",
		);

		if (element) {
			element.classList.add("flex", "items-center");
			setHasTitle(true);

			if (element.children.length === 0) {
				const language = element.getAttribute("data-language") || "";
				const Icon = icons[language as keyof typeof icons];
				console.log(Icon, language, icons);
				if (!Icon) {
					console.error(`No icon found for language: ${language}`);
					return;
				}
				const iconElement = <Icon className="w-6 h-6 mr-2" />;
				const rendered = renderToStaticMarkup(iconElement);
				const htmlIcon = document.createElement("svg");
				htmlIcon.innerHTML = rendered;

				element.prepend(htmlIcon);
			}
		}
	}, []);

	return (
		<div className="relative group not-prose" ref={divRef}>
			<pre
				{...props}
				ref={preRef}
				className={cn("overflow-auto", {
					"rounded-t-none border-t-transparent": hasTitle,
				})}
			>
				{children}
			</pre>
			<div className="absolute right-2 top-2">
				<button
					type="button"
					aria-label={"Copy code to clipboard"}
					className="md:hidden group-hover:opacity-100 appearance-none border dark:border-gray-700 border-gray-200 cursor-pointer dark:bg-[--shiki-background] rounded-md dark:text-gray-400 opacity-0 select-none py-2 px-2 transition-opacity duration-200 ease-in-out bg-white text-gray-800"
					onClick={toggleWordWrap}
				>
					<WrapText className="w-4 h-4" />
				</button>
				<button
					type="button"
					aria-label={"Copy code to clipboard"}
					className="group-hover:opacity-100 appearance-none border dark:border-gray-700 border-gray-200 cursor-pointer dark:bg-[--shiki-background] rounded-md dark:text-gray-400 opacity-0 select-none py-2 px-2 transition-opacity duration-200 ease-in-out bg-white text-gray-800"
					onClick={handleCopyCode}
				>
					{showCopied ? (
						<Check className="w-4 h-4" />
					) : (
						<Clipboard className="w-4 h-4" />
					)}
				</button>
			</div>
		</div>
	);
};

export default Pre;
