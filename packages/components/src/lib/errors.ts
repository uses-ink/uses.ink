import { MESSAGES_404, MESSAGES_500 } from "@uses.ink/constants";

export const getRandom404Message = () => {
	return MESSAGES_404[Math.floor(Math.random() * MESSAGES_404.length)];
};

export const getRandom500Message = () => {
	return MESSAGES_500[Math.floor(Math.random() * MESSAGES_500.length)];
};
