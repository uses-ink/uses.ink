interface NavbarProps {
	routes: {
		[key: string]: string;
	};
}

export const Navbar = ({ routes }: NavbarProps) => {
	return (
		<nav className="flex justify-between items-center p-4 fixed top-0 left-0 right-0 z-10 w-fit">
			<div className="flex items-center space-x-3">
				{/* Join items with · */}
				{Object.entries(routes).map(([name, route], index) => (
					<>
						<a key={index.toString()} href={route} className="font-bold">
							{name}
						</a>
						{index < Object.entries(routes).length - 1 && (
							<span className="text-gray-400 dark:text-gray-500">·</span>
						)}
					</>
				))}
			</div>
		</nav>
	);
};
