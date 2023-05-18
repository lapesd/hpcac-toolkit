import LoginForm from "../components/LoginForm.tsx";
import Dashboard from "../components/Dashboard.tsx";

import type { Handlers, PageProps } from "$fresh/server.ts";
import { getCookies } from "std/http/cookie.ts";

interface Data {
  isAllowed: boolean;
}

export const handler: Handlers = {
  GET(req, ctx) {
    const cookies = getCookies(req.headers);
    return ctx.render!({ isAllowed: cookies.auth === "bar" });
  },
};

export default function Home({ data }: PageProps<Data>) {
  return (
    <div>
      {!data.isAllowed
        ? <LoginForm isAllowed={data.isAllowed} />
        : <Dashboard />}
    </div>
  );
}
