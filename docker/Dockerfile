FROM oven/bun:1.0.35-alpine AS base
RUN apk add --no-cache bash git curl make jq tini nodejs

WORKDIR /app

FROM base AS prune

COPY . .

RUN ./scripts/prune.sh

FROM base AS prod-deps

COPY --from=prune /app/pruned/ ./pruned/
COPY package.json bun.lockb ./pruned/
COPY patches ./pruned/patches
COPY scripts ./pruned/scripts

WORKDIR /app/pruned

RUN bun install --frozen-lockfile --production

FROM base AS build-deps

COPY --from=prune /app/pruned/ ./pruned
COPY package.json bun.lockb ./pruned/
COPY patches ./pruned/patches
COPY scripts ./pruned/scripts

WORKDIR /app/pruned

RUN bun install --frozen-lockfile

FROM base AS build


COPY . .

COPY --from=build-deps /app/pruned/node_modules ./node_modules

WORKDIR /app/apps/web

RUN bunx astro telemetry disable
RUN bunx astro build

FROM base AS runtime
COPY --from=prod-deps /app/pruned/node_modules ./node_modules
COPY --from=build /app/apps/web/dist ./dist

ENV HOST=0.0.0.0
ENV PORT=8765

EXPOSE 8765

ENTRYPOINT ["/sbin/tini", "--"]
# For some reason, bun runtime doesn't yet work with astro
CMD node ./dist/server/entry.mjs