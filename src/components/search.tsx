import { useState, useRef, useEffect, type ComponentType } from "react";
import Mark from "mark.js";
import { SearchIcon } from "lucide-react";
import { Input } from "./ui/input";
import { cn } from "@/lib/utils";

interface ContentComponentProps {
	[key: string]: any;
}

interface SearchableContentProps<T extends ContentComponentProps> {
	ContentComponent: ComponentType<T>;
	contentProps: T;
}

function SearchableContent<T extends ContentComponentProps>({
	ContentComponent,
	contentProps,
}: SearchableContentProps<T>): React.ReactElement {
	const [searchTerm, setSearchTerm] = useState<string>("");
	const [matchCount, setMatchCount] = useState<number>(0);
	const contentRef = useRef<HTMLDivElement | null>(null);
	const markInstance = useRef<Mark | null>(null);

	useEffect(() => {
		if (contentRef.current) {
			markInstance.current = new Mark(contentRef.current);
		}
	}, []);

	useEffect(() => {
		if (markInstance.current) {
			markInstance.current.unmark({
				done: () => {
					if (searchTerm) {
						markInstance.current?.mark(searchTerm, {
							separateWordSearch: false,
							done: (totalMarks: number) => {
								setMatchCount(totalMarks);
								if (totalMarks > 0) {
									scrollToFirstMatch();
								}
							},
						});
					} else {
						setMatchCount(0);
					}
				},
			});
		}
	}, [searchTerm]);

	const scrollToFirstMatch = () => {
		const firstMatch = contentRef.current?.querySelector("mark");
		if (firstMatch) {
			firstMatch.scrollIntoView({ behavior: "smooth", block: "center" });
		}
	};

	const [expanded, setExpanded] = useState(false);
	const inputRef = useRef<HTMLInputElement | null>(null);

	return (
		<>
			<div className="fixed top-0 right-0 p-4">
				<div className="relative">
					<Input
						value={searchTerm}
						onChange={(e) => setSearchTerm(e.target.value)}
						// placeholder="Search"
						className={cn("transition-all w-0", expanded ? "w-48" : "w-12")}
						onFocus={() => setExpanded(true)}
						onBlur={() => setExpanded(false)}
						ref={inputRef}
					/>
					<SearchIcon
						className="w-4 h-4 absolute top-1/2 right-2 transform -translate-y-1/2 mr-2 cursor-text"
						onClick={() => inputRef.current?.focus()}
					/>
				</div>
			</div>
			<div ref={contentRef}>
				<ContentComponent {...contentProps} />
			</div>
		</>
	);
}

export default SearchableContent;
