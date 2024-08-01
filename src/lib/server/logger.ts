import pino from "pino";

export const serverLogger = pino().child({ type: "server" });
