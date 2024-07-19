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
		<pre {...props} ref={pre}>
			{children}
			<button
				type="button"
				aria-label={"Copy code to clipboard"}
				className="copy-button"
				onClick={handleCopyCode}
			>
				{showCopied ? <Check /> : <Clipboard />}
			</button>
		</pre>
	);
};

export default CodeBlock;
