# syntax=docker/dockerfile:1

# -----------------------------------------------------------------------------
# React/Vite tester build
# -----------------------------------------------------------------------------
FROM node:22-alpine AS build

WORKDIR /app
COPY package.json package-lock.json ./
RUN npm ci --no-audit --no-fund

COPY . .
RUN npm run build

# -----------------------------------------------------------------------------
# Static runtime: Nginx serves the SPA and proxies /api/ to the Rust service.
# -----------------------------------------------------------------------------
FROM nginx:1.27-alpine AS runtime

COPY nginx.conf /etc/nginx/conf.d/default.conf
COPY --from=build /app/dist /usr/share/nginx/html

EXPOSE 80

HEALTHCHECK --interval=30s --timeout=4s --start-period=5s --retries=3 \
    CMD ["wget", "-qO-", "http://127.0.0.1/"]
