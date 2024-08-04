import { GalleryImage, mdxComponents } from "@/lib/client/mdx/components";
import { GalleryLayout } from "./gallery";
import { PostLayout } from "./post";
import { clientLogger } from "@/lib/client/logger";

export const getLayout = (layout: string, Content: React.FC<any>) => {
	let Layout = PostLayout;
	switch (layout) {
		case "gallery":
			Layout = GalleryLayout;
			break;

		default:
			Layout = PostLayout;
	}

	clientLogger.debug("getLayout -> Layout", Layout);
	clientLogger.info("getLayout -> Content", Content);

	return () => (
		<Layout>
			<Content
				components={{
					...mdxComponents,
					...(layout === "gallery" ? { img: GalleryImage as any } : {}),
				}}
			/>
		</Layout>
	);
};
