import ThemeSelect from "./theme-select";

export const Footer = () => {
	return (
		<div className="flex justify-center mb-12 text-xs">
			<div className="flex gap-4 items-center justify-between">
				<ThemeSelect />
				<p className="whitespace-nowrap !text-muted-foreground">
					Made with{" "}
					<a href="https://uses.ink" target="_blank" rel="noreferrer">
						uses.ink
					</a>
				</p>
			</div>
		</div>
	);
};
