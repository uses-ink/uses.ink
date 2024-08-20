# syntax=docker.io/docker/dockerfile:1.9-labs
FROM node:20-alpine AS base


ENV PNPM_HOME="/pnpm"
ENV PATH="$PNPM_HOME:$PATH"
RUN corepack enable
RUN apk add --no-cache bash git curl make
RUN curl -fsSL https://d2lang.com/install.sh | sh -s --
# Check if d2 has been installed
RUN d2 --version

# Install dependencies only when needed
FROM base AS deps
# Rebuild the source code only when needed
FROM base AS builder
WORKDIR /app

COPY . .
# Check https://github.com/nodejs/docker-node/tree/b4117f9333da4138b03a546ec926ef50a31506c3#nodealpine to understand why libc6-compat might be needed.
RUN apk add --no-cache libc6-compat

RUN --mount=type=cache,id=pnpm,target=/pnpm/store pnpm i --frozen-lockfile

ENV NEXT_TELEMETRY_DISABLED=1

RUN pnpm run build

# Production image, copy all the files and run next
FROM base AS runner
WORKDIR /app

ENV NODE_ENV=production

ENV NEXT_TELEMETRY_DISABLED=1

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

COPY apps/web/docker-entrypoint.sh .
COPY apps/web/.env.production .

RUN ["chmod", "755", "./docker-entrypoint.sh"]
ENTRYPOINT ["./docker-entrypoint.sh"]

USER nextjs

EXPOSE 8765

ENV PORT=8765

# server.js is created by next build from the standalone output
# https://nextjs.org/docs/pages/api-reference/next-config-js/output
CMD ["node", "server.js"]