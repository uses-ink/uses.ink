import { Footer } from "./footer";

const Article = ({
	children,
	auto,
}: { children: React.ReactNode; auto?: boolean }) => (
	<article
		className="container mx-auto xl:prose-lg prose max-md:prose-sm dark:prose-invert"
		dir="ltr"
	>
		{children}
		<hr className="!mb-4" />
		<Footer {...{ auto }} />
	</article>
);

export default Article;
