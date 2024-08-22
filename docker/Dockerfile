FROM oven/bun:canary-alpine AS base
RUN apk add --no-cache bash git curl make jq

WORKDIR /app

FROM base AS prune

COPY . .

RUN ./scripts/prune.sh

FROM base as prod-deps

COPY --from=prune /app/pruned/ ./pruned/
COPY package.json package-lock.json ./pruned/

WORKDIR /app/pruned

RUN bun install --frozen-lockfile --production

FROM base AS build-deps

COPY --from=prune /app/pruned/ ./pruned
COPY package.json package-lock.json ./pruned/

WORKDIR /app/pruned

RUN bun install --frozen-lockfile

FROM base AS build

COPY . .

COPY --from=build-deps /app/pruned/node_modules ./node_modules

RUN ["npm", "run", "build", "-w", "apps/web"]

FROM base AS runtime
COPY --from=prod-deps /app/pruned/node_modules ./node_modules
COPY --from=build /app/apps/web/dist ./dist

ENV HOST=0.0.0.0
ENV PORT=8765

EXPOSE 8765

CMD node ./dist/server/entry.mjs