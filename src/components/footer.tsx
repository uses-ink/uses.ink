import ThemeSelect from "./theme-select";
import {
	Tooltip,
	TooltipContent,
	TooltipProvider,
	TooltipTrigger,
} from "./ui/tooltip";

export const Footer = () => {
	return (
		<div className="flex justify-center mb-12 text-xs">
			<div className="flex gap-4 items-center justify-between">
				<ThemeSelect />
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
						<TooltipContent>
							{process.env.NEXT_PUBLIC_BUILD_VERSION ?? "v0.0.0"}{" "}
							<span className="dark:text-gray-500 text-gray-300">
								({process.env.NEXT_PUBLIC_BUILD_COMMIT})
							</span>
						</TooltipContent>
					</Tooltip>
				</TooltipProvider>
			</div>
		</div>
	);
};
