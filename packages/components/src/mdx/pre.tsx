"use client";

import { logger } from "@uses.ink/client-logger";
import { Check, Clipboard, WrapText } from "lucide-react";
import { cn } from "../lib/utils";

const Pre = ({
	children,
	...props
}: {
	children: React.ReactNode;
}) => {
	return (
		<div className="relative group not-prose" id="code-parent">
			<pre {...props} className="overflow-auto">
				{children}
			</pre>
			<div className="absolute right-2 top-2">
				<button
					type="button"
					aria-label={"Toggle word wrap"}
					className="md:hidden mr-1 group-hover:opacity-100 appearance-none border dark:border-gray-700 border-gray-200 cursor-pointer bg-[--shiki-background] rounded-md dark:text-gray-400 opacity-0 select-none py-2 px-2 transition-opacity duration-200 ease-in-out text-gray-800"
					id="toggle-wrap"
				>
					<WrapText className="w-4 h-4" />
				</button>
				<button
					type="button"
					aria-label={"Copy code to clipboard"}
					className="group-hover:opacity-100 appearance-none border dark:border-gray-700 border-gray-200 cursor-pointer bg-[--shiki-background] rounded-md dark:text-gray-400 opacity-0 select-none py-2 px-2 transition-opacity duration-200 ease-in-out text-gray-800"
					id="copy-code"
				>
					<Check className="w-4 h-4 hidden" id="check" />

					<Clipboard className="w-4 h-4" id="clip" />
				</button>
			</div>
		</div>
	);
};

export default Pre;
