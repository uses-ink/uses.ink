import { useState, useRef, useEffect, type ComponentType } from "react";
import Mark from "mark.js";
import { ChevronDownIcon, ChevronUpIcon, SearchIcon } from "lucide-react";
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
	const [currentMatchIndex, setCurrentMatchIndex] = useState<number>(0);

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
		const matches = contentRef.current?.querySelectorAll("mark");
		if (matches && matches.length > 0) {
			matches[0].scrollIntoView({
				behavior: "smooth",
				block: "center",
			});
			setCurrentMatchIndex(0);
		}
	};

	const scrollToNextMatch = () => {
		const matches = contentRef.current?.querySelectorAll("mark");
		if (matches && matches.length > 0) {
			const nextMatchIndex = (currentMatchIndex + 1) % matches.length;
			matches[nextMatchIndex].scrollIntoView({
				behavior: "smooth",
				block: "center",
			});
			setCurrentMatchIndex(nextMatchIndex);
		}
	};

	const scrollToPreviousMatch = () => {
		const matches = contentRef.current?.querySelectorAll("mark");
		if (matches && matches.length > 0) {
			const previousMatchIndex =
				(currentMatchIndex - 1 + matches.length) % matches.length;
			matches[previousMatchIndex].scrollIntoView({
				behavior: "smooth",
				block: "center",
			});
			setCurrentMatchIndex(previousMatchIndex);
		}
	};

	const scrollToMatch = (index: number) => {
		const matches = contentRef.current?.querySelectorAll("mark");
		if (matches && matches.length > 0) {
			const matchIndex = (index + matches.length) % matches.length;
			matches[matchIndex].scrollIntoView({
				behavior: "smooth",
				block: "center",
			});
			setCurrentMatchIndex(matchIndex);
		}
	};

	const [expanded, setExpanded] = useState(false);
	const inputRef = useRef<HTMLInputElement | null>(null);

	return (
		<>
			<div className="fixed top-0 right-0 p-4 flex gap-2">
				{matchCount > 0 && (
					<div className="flex gap-2 items-center">
						<p className="not-prose text-sm flex items-center justify-between">
							<input
								className="appearance-none bg-transparent border-none w-4 max-w-8 focus:outline-none remove-arrow text-right"
								value={currentMatchIndex === -1 ? "" : currentMatchIndex + 1}
								type="number"
								onChange={(e) => {
									if (!e.target.value) {
										setCurrentMatchIndex(-1);
									}
									const value = +e.target.value;
									if (value >= 1 && value <= matchCount) {
										setCurrentMatchIndex(value - 1);
										scrollToMatch(value - 1);
									}
								}}
							/>
							<span className="mx-1">/</span>
							<span>{matchCount}</span>
						</p>
						<div className="flex flex-col gap-1 mx-2">
							<ChevronUpIcon
								className="w-4 h-4 cursor-pointer"
								onClick={() => {
									inputRef.current?.focus();
									scrollToPreviousMatch();
								}}
							/>
							<ChevronDownIcon
								className="w-4 h-4 cursor-pointer"
								onClick={() => {
									inputRef.current?.focus();
									scrollToNextMatch();
								}}
							/>
						</div>
					</div>
				)}
				<div className="relative">
					<Input
						value={expanded ? searchTerm : ""}
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
