// Based on https://github.com/facebook/docusaurus/blob/ed9d2a26f5a7b8096804ae1b3a4fffc504f8f90d/packages/docusaurus-theme-classic/src/theme/CodeBlock/index.tsx
// which is under MIT License as per the banner

import { useRef, useState } from "react";
import { Clipboard, Check } from "lucide-react";
const CodeBlock = ({
	children,
	...props
}: {
	children: React.ReactNode;
}) => {
	const pre = useRef<HTMLPreElement>(null);
	const [showCopied, setShowCopied] = useState(false);

	const handleCopyCode = async () => {
		if (pre.current) {
			const content = Array.from(pre.current.querySelectorAll("code span.line"))
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

	return (
		<div className="relative group">
			<pre {...props} ref={pre}>
				{children}
			</pre>
			<button
				type="button"
				aria-label={"Copy code to clipboard"}
				className="group-hover:opacity-100 appearance-none border dark:border-gray-700 border-gray-200 cursor-pointer dark:bg-[--shiki-background] rounded-md dark:text-gray-400 opacity-0 select-none py-2 px-2 absolute right-2 top-2 transition-opacity duration-200 ease-in-out bg-white text-gray-800"
				onClick={handleCopyCode}
			>
				{showCopied ? (
					<Check className="w-4 h-4" />
				) : (
					<Clipboard className="w-4 h-4" />
				)}
			</button>
		</div>
	);
};

export default CodeBlock;
