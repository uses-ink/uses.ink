import { LucideLoader2 } from "lucide-react";

export const Loading = () => {
	return (
		<div className="flex w-screen justify-center items-center prose max-w-full dark:prose-invert">
			<div className="text-center flex gap-2 flex-col items-center">
				<LucideLoader2 className="w-16 h-16 animate-spin" />
			</div>
		</div>
	);
};
