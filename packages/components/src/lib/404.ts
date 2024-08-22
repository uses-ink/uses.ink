import { MESSAGES_404 } from "@uses.ink/constants";

export const getRandom404Message = () => {
	return MESSAGES_404[Math.floor(Math.random() * MESSAGES_404.length)];
};
