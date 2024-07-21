# syntax=docker.io/docker/dockerfile:1.7-labs

FROM node:18-alpine AS base

ENV PNPM_HOME="/pnpm"
ENV PATH="$PNPM_HOME:$PATH"
RUN corepack enable
RUN apk add --no-cache bash

# Install dependencies only when needed
FROM base AS deps
# Check https://github.com/nodejs/docker-node/tree/b4117f9333da4138b03a546ec926ef50a31506c3#nodealpine to understand why libc6-compat might be needed.
RUN apk add --no-cache libc6-compat
WORKDIR /app

# Install dependencies based on the preferred package manager
COPY package.json pnpm-lock.yaml* ./
COPY patches ./patches
RUN --mount=type=cache,id=pnpm,target=/pnpm/store pnpm i --frozen-lockfile


# Rebuild the source code only when needed
FROM base AS builder
WORKDIR /app
COPY --from=deps /app/node_modules ./node_modules
COPY --exclude=docker-entrypoint.sh --exclude=.env.production . .

ENV NEXT_TELEMETRY_DISABLED 1

RUN pnpm run build

# Production image, copy all the files and run next
FROM base AS runner
WORKDIR /app
COPY blog ./blog

ENV NODE_ENV production

ENV NEXT_TELEMETRY_DISABLED 1

RUN addgroup --system --gid 1001 nodejs
RUN adduser --system --uid 1001 nextjs

COPY --from=builder /app/public ./public

# Set the correct permission for prerender cache
RUN mkdir .next
RUN chown nextjs:nodejs .next

# Automatically leverage output traces to reduce image size
# https://nextjs.org/docs/advanced-features/output-file-tracing
COPY --from=builder --chown=nextjs:nodejs /app/.next/standalone ./
COPY --from=builder --chown=nextjs:nodejs /app/.next/static ./.next/static

COPY docker-entrypoint.sh .
COPY .env.production .

RUN ["chmod", "755", "./docker-entrypoint.sh"]
ENTRYPOINT ["./docker-entrypoint.sh"]

USER nextjs

EXPOSE 3000

ENV PORT 3000

# server.js is created by next build from the standalone output
# https://nextjs.org/docs/pages/api-reference/next-config-js/output
CMD HOSTNAME="0.0.0.0" node server.js