FROM denoland/deno:1.33.3

EXPOSE 4000

WORKDIR /app

ADD . /app

# Add dependencies to the container's Deno cache
RUN deno cache main.ts --import-map=import_map.json

CMD ["run", "--allow-run", "--allow-write", "--allow-read", "--allow-env", "--allow-net", "main.ts"]
