# Build the frontend, then serve it from nginx.
FROM node:20-alpine AS build
ARG NODE_ENV=production
WORKDIR /app
ENV NEXT_TELEMETRY_DISABLED=1 PORT=3000
COPY package.json ./
RUN npm install \
    && npm run build

FROM nginx:1.27-alpine
COPY --from=build /app/dist /usr/share/nginx/html
USER nginx
VOLUME ["/var/cache/nginx"]
EXPOSE 80 443
ENTRYPOINT ["nginx"]
CMD ["-g", "daemon off;"]
