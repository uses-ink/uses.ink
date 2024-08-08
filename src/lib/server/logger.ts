import pino from "pino";

const level = process.env.NODE_ENV === "production" ? "info" : "debug";

export const serverLogger = pino({ level }).child({ type: "server" });
