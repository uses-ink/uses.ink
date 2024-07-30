export function GalleryLayout({ children }: { children: React.ReactNode }) {
	return (
		// spread the children to the grid horizontally
		<div className="grid grid-cols-1 md:grid-cols-2 gap-4">{children}</div>
	);
}
