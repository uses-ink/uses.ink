FROM node:lts-alpine AS base
ENV PNPM_HOME="/pnpm"
ENV PATH="$PNPM_HOME:$PATH"
RUN corepack enable
RUN apk add --no-cache bash git curl make

WORKDIR /app

COPY package.json pnpm-lock.yaml patches/ ./

FROM base AS prod-deps
RUN --mount=type=cache,id=pnpm,target=/pnpm/store pnpm i --frozen-lockfile --prod

FROM base AS build-deps
RUN --mount=type=cache,id=pnpm,target=/pnpm/store pnpm i --frozen-lockfile

FROM build-deps AS build
COPY . .
RUN pnpm run build

FROM base AS runtime
COPY --from=prod-deps /app/apps/web/node_modules ./node_modules
COPY --from=build /app/apps/web/dist ./dist

ENV HOST=0.0.0.0
ENV PORT=4321

EXPOSE 4321

CMD node ./dist/server/entry.mjs