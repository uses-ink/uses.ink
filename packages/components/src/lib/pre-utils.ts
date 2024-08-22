const copyButtons = document.querySelectorAll("#copy-code");
// biome-ignore lint/complexity/noForEach: <explanation>
copyButtons.forEach((button) => {
	button.addEventListener("click", () => {
		// Get the closest #code-parent element
		const parent = button.closest("#code-parent");
		if (!parent) return;
		const code = parent.querySelector("code") as HTMLElement;
		const check = parent.querySelector("#check") as HTMLElement;
		const clip = parent.querySelector("#clip") as HTMLElement;
		if (!code || !check || !clip) return;
		check.style.display = "block";
		clip.style.display = "none";
		setTimeout(() => {
			check.style.display = "none";
			clip.style.display = "block";
		}, 3000);

		navigator.clipboard.writeText(code.innerText);
	});
});
const wrapButtons = document.querySelectorAll("#toggle-wrap");
// biome-ignore lint/complexity/noForEach: <explanation>
wrapButtons.forEach((button) => {
	button.addEventListener("click", () => {
		const htmlDataset = document.documentElement.dataset;
		const hasWordWrap = "wordWrap" in htmlDataset;
		if (hasWordWrap) {
			// biome-ignore lint/performance/noDelete: need to delete
			delete htmlDataset.wordWrap;
		} else {
			htmlDataset.wordWrap = "";
		}
	});
});
