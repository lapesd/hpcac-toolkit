import { Client } from "https://deno.land/x/postgres@v0.17.0/mod.ts";

async function Connect() {
  const client = new Client({
    user: "username",
    database: "database_name",
    hostname: "hostname",
    port: 5432,
    password: "password",
  });

  await client.connect();

  return client;
}
