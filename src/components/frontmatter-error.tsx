const FrontmatterError = ({
	errors,
}: {
	errors: any;
}) => (
	<div className="flex w-screen justify-center items-center prose max-w-full dark:prose-invert">
		<div className="text-center flex gap-2 flex-col items-center">
			<h1 className="text-4xl">
				An error occured when parsing your frontmatter.
			</h1>
			<pre className="text-left language-json">
				{JSON.stringify(errors, null, 2)}
			</pre>
			<p className="text-lg dark:text-gray-400 text-gray-600">
				See the{" "}
				<a
					href="https://uses.ink/features#metadata"
					target="_blank"
					rel="noreferrer"
				>
					frontmatter documentation
				</a>{" "}
				for more information.
			</p>
			<h3>
				<a href="https://uses.ink">Back to uses.ink</a>
			</h3>
		</div>
	</div>
);
