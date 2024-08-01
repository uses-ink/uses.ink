import pino from "pino";

export const clientLogger = pino().child({ type: "client" });
