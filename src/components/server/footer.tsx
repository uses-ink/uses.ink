import ThemeSelect from "../client/theme-select";
import {
	Tooltip,
	TooltipContent,
	TooltipProvider,
	TooltipTrigger,
} from "../ui/tooltip";

export const Footer = ({
	auto,
}: {
	auto?: boolean;
}) => {
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
							<a
								href={
									process.env.NEXT_PUBLIC_BUILD_VERSION?.includes(
										process.env.NEXT_PUBLIC_BUILD_COMMIT?.slice(0, 7) ?? "",
									)
										? `https://github.com/uses-ink/uses.ink/commit/${process.env.NEXT_PUBLIC_BUILD_COMMIT?.slice(0, 7)}`
										: `https://github.com/uses-ink/uses.ink/releases/tag/${process.env.NEXT_PUBLIC_BUILD_VERSION}`
								}
								target="_blank"
								rel="noreferrer"
								className="not-prose"
							>
								{process.env.NEXT_PUBLIC_BUILD_VERSION ?? "v0.0.0"}{" "}
								<span className="dark:text-gray-500 text-gray-300">
									({process.env.NEXT_PUBLIC_BUILD_COMMIT?.slice(0, 7)})
								</span>
							</a>
						</TooltipContent>
					</Tooltip>
				</TooltipProvider>
				{auto && (
					<a
						href="https://uses.ink/features.md#auto-generated-readme"
						className="whitespace-nowrap !text-muted-foreground not-prose"
					>
						<p className="whitespace-nowrap !text-muted-foreground">
							Automatically generated
						</p>
					</a>
				)}
			</div>
		</div>
	);
};
