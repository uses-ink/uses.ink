import Img from "@/components/client/img";
import Pre from "@/components/client/pre";
import Link from "next/link";

// To have client-side routing
const CustomLink = (props: any) =>
	props.href.startsWith("#") ? <a {...props} /> : <Link {...props} />;

/**
 * Use Next's components in MDX (e.g. Head)
 */
export const mdxComponents = {
	a: CustomLink,
	// https://github.com/mdx-js/mdx/discussions/1921
	pre: Pre as any,
	img: Img as any,
	h1: ({ children, ...props }: any) => (
		<h1 {...props} className="group">
			{children}
			<Anchor id={props.id} />
		</h1>
	),
	h2: ({ children, ...props }: any) => (
		<h2 {...props} className="group">
			{children}
			<Anchor id={props.id} />
		</h2>
	),
	h3: ({ children, ...props }: any) => (
		<h3 {...props} className="group">
			{children}
			<Anchor id={props.id} />
		</h3>
	),
	h4: ({ children, ...props }: any) => (
		<h4 {...props} className="group">
			{children}
			<Anchor id={props.id} />
		</h4>
	),
	h5: ({ children, ...props }: any) => (
		<h5 {...props} className="group">
			{children}
			<Anchor id={props.id} />
		</h5>
	),
	h6: ({ children, ...props }: any) => (
		<h6 {...props} className="group">
			{children}
			<Anchor id={props.id} />
		</h6>
	),
};

const Anchor = ({ id }: any) => (
	<a
		href={`#${id}`}
		className="anchor !text-muted-foreground opacity-0 group-hover:opacity-100 transition-opacity duration-200"
		style={{ marginLeft: "0.5rem", textDecoration: "none" }}
	>
		#
	</a>
);

export const GalleryImage = ({ src, alt }: { src: string; alt: string }) => (
	<div className="flex flex-col items-center not-prose gap-2">
		<img src={src} alt={alt} className="rounded-lg" />
		{alt && <p className="text-sm !text-muted-foreground">{alt}</p>}
	</div>
);
