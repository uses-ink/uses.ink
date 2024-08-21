###########################
#     BASE CONTAINER      #
###########################
FROM node:20-alpine AS base
ENV PNPM_HOME="/pnpm"
ENV PATH="$PNPM_HOME:$PATH"
RUN corepack enable
RUN apk add --no-cache bash git curl make
RUN curl -fsSL https://d2lang.com/install.sh | sh -s --
# Check if d2 has been installed
RUN d2 --version

###########################
#    BUILDER CONTAINER    #
###########################
FROM base AS builder

# Check https://github.com/nodejs/docker-node/tree/b4117f9333da4138b03a546ec926ef50a31506c3#nodealpine to understand why libc6-compat might be needed.
RUN apk add --no-cache libc6-compat
RUN apk add --no-cache jq
WORKDIR /app

COPY . .

# Get the turbo version from package.json
RUN TURBO_VERSION=$(jq -r '.devDependencies.turbo' package.json)

RUN pnpm install -g "turbo@$TURBO_VERSION"

# Outputs to the /out folder
# source: https://turbo.build/repo/docs/reference/command-line-reference/prune#--docker
RUN turbo prune --scope=@uses.ink/web --docker

###########################
#   INSTALLER CONTAINER   #
###########################
FROM base AS installer

# Check https://github.com/nodejs/docker-node/tree/b4117f9333da4138b03a546ec926ef50a31506c3#nodealpine to understand why libc6-compat might be needed.
RUN apk add --no-cache libc6-compat
RUN apk add --no-cache jq
# Required for node_modules/aws-crt
RUN apk add --no-cache make cmake g++

WORKDIR /app

# Disable husky from installing hooks
ENV NEXT_TELEMETRY_DISABLED 1

# First install the dependencies (as they change less often)
COPY .gitignore .gitignore
COPY --from=builder /app/out/json/ .
COPY --from=builder /app/out/pnpm-lock.yaml .

RUN --mount=type=cache,id=pnpm,target=/pnpm/store pnpm i --frozen-lockfile

# Then copy all the source code (as it changes more often)
COPY --from=builder /app/out/full/ .
# Finally copy the turbo.json file so that we can run turbo commands
COPY turbo.json turbo.json

RUN TURBO_VERSION=$(jq -r '.devDependencies.turbo' package.json)

RUN pnpm install -g "turbo@$TURBO_VERSION"

RUN turbo run build --filter=@uses.ink/web...

###########################
#     RUNNER CONTAINER    #
###########################
FROM base AS runner

WORKDIR /app

# Don't run production as root
RUN addgroup --system --gid 1001 nodejs
RUN adduser --system --uid 1001 nextjs

USER nextjs

COPY --from=installer /app/apps/web/next.config.js .
COPY --from=installer /app/apps/web/package.json .

# Automatically leverage output traces to reduce image size
# https://nextjs.org/docs/advanced-features/output-file-tracing
COPY --from=installer --chown=nextjs:nodejs /app/apps/web/.next/standalone ./
COPY --from=installer --chown=nextjs:nodejs /app/apps/web/.next/static ./apps/web/.next/static
COPY --from=installer --chown=nextjs:nodejs /app/apps/web/public ./apps/web/public

COPY docker/docker-entrypoint.sh .
COPY apps/web/.env.production .

RUN ["chmod", "755", "./docker-entrypoint.sh"]
ENTRYPOINT ["./docker-entrypoint.sh"]

EXPOSE 8765

ENV PORT=8765

# server.js is created by next build from the standalone output
# https://nextjs.org/docs/pages/api-reference/next-config-js/output
CMD ["node", "apps/web/server.js"]