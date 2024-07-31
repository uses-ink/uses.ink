import { GalleryImage, mdxComponents } from "@/lib/client/mdx/components";
import { GalleryLayout } from "./gallery";
import { PostLayout } from "./post";

export const getLayout = (layout: string, Content: React.FC<any>) => {
	let Layout = PostLayout;
	switch (layout) {
		case "gallery":
			Layout = GalleryLayout;
			break;
		default:
			Layout = PostLayout;
	}

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
