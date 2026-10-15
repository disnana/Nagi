import { expect, test } from "@playwright/test";
import { resetFixture } from "./control";
import {
  lastSequence,
  latestFor,
  readObservations,
  waitForNew,
  type Observation,
} from "./observations";

const appOrigin = new URL(process.env.SF08_APP_ORIGIN!).origin;
const uiOrigin = new URL(process.env.SF08_UI_ORIGIN!).origin;
const deniedOrigin = new URL(process.env.SF08_DENIED_ORIGIN!).origin;

function expectManaged(event: Observation, status: number): void {
  expect(event.method).toBe("GET");
  expect(event.path).toBe("/bootstrap");
  expect(event.status).toBe(status);
  expect(event.allow_origin_present).toBe(false);
  expect(event.origin_class).toBe("same_origin");
}

test.beforeEach(async ({ browser }) => {
  await resetFixture();
  test.info().annotations.push({
    type: "browser-version",
    description: `${browser.browserType().name()} ${browser.version()}`,
  });
});

test("TLS bootstrap sets the strict prelogin cookie and managed response headers", async ({ page, context }) => {
  const documentResponse = await page.goto(`${appOrigin}/`);
  expect(documentResponse, "TLS fixture must serve the controlled root page").not.toBeNull();
  expect(documentResponse!.status()).toBe(200);
  expect(new URL(page.url()).protocol).toBe("https:");
  const before = await readObservations();
  const beforeSeq = lastSequence(before);
  const bootstrap = await page.evaluate(async () => {
    const response = await fetch("/bootstrap", { credentials: "same-origin" });
    return {
      status: response.status,
      cacheControl: response.headers.get("cache-control"),
      vary: response.headers.get("vary"),
      body: await response.json(),
    };
  });
  expect(bootstrap.status).toBe(200);
  expect(bootstrap.cacheControl).toBe("no-store");
  expect(bootstrap.vary).toBe("Authorization, Content-Type, Cookie, Origin, X-CSRF-Token");
  const csrfToken = bootstrap.body.csrf_token;
  expect(typeof csrfToken === "string" && csrfToken.length > 0).toBe(true);

  const observed = await waitForNew(beforeSeq, 1, (event) =>
    event.fixture === "login" && event.method === "GET" && event.path === "/bootstrap",
  );
  expectManaged(observed[0], 200);

  const cookie = (await context.cookies(appOrigin)).find((value) => value.name === "__Host-prelogin");
  expect(cookie).toBeDefined();
  expect(cookie?.domain).toBe("localhost");
  expect(cookie?.path).toBe("/");
  expect(cookie?.secure).toBe(true);
  expect(cookie?.httpOnly).toBe(true);
  expect(cookie?.sameSite).toBe("Strict");
});

test("browser CSRF rejection preserves a claim and isolated replay refuses the consumed cookie", async ({ browser, page, context }) => {
  const documentResponse = await page.goto(`${appOrigin}/`);
  expect(documentResponse?.status()).toBe(200);
  expect(new URL(page.url()).protocol).toBe("https:");
  const before = await readObservations();
  const beforeSeq = lastSequence(before);
  const countersBefore = latestFor(before, "login");
  const bootstrap = await page.evaluate(async () => {
    const response = await fetch("/bootstrap", { credentials: "same-origin" });
    return { status: response.status, body: await response.json() };
  });
  expect(bootstrap.status).toBe(200);
  const bootstrapEvent = await waitForNew(beforeSeq, 1, (event) =>
    event.fixture === "login" && event.method === "GET" && event.path === "/bootstrap",
  );
  const csrfToken = bootstrap.body.csrf_token;
  const csrfTokenIsNonemptyString = typeof csrfToken === "string" && csrfToken.length > 0;
  expect(csrfTokenIsNonemptyString).toBe(true);
  if (typeof csrfToken !== "string") throw new Error("SF08 bootstrap did not return a CSRF token");
  const originalCookie = (await context.cookies(appOrigin)).find((cookie) => cookie.name === "__Host-prelogin");
  expect(originalCookie).toBeDefined();
  const originalCookieSnapshot = {
    name: originalCookie!.name,
    value: originalCookie!.value,
    domain: originalCookie!.domain,
    path: originalCookie!.path,
    expires: originalCookie!.expires,
    httpOnly: originalCookie!.httpOnly,
    secure: originalCookie!.secure,
    sameSite: originalCookie!.sameSite,
  };
  expect(bootstrapEvent[0].ready_rows).toBe((countersBefore?.ready_rows ?? 0) + 1);

  const rejected = await page.evaluate(async () => {
    const response = await fetch("/login", {
      method: "POST",
      credentials: "same-origin",
      headers: { Authorization: "Bearer owned", "Content-Type": "application/json" },
      body: "{}",
    });
    return {
      status: response.status,
      cacheControl: response.headers.get("cache-control"),
      vary: response.headers.get("vary"),
    };
  });
  expect(rejected).toEqual({
    status: 403,
    cacheControl: "no-store",
    vary: "Authorization, Content-Type, Cookie, Origin, X-CSRF-Token",
  });
  const csrfEvent = await waitForNew(bootstrapEvent[0].seq, 1, (event) =>
    event.fixture === "login" && event.method === "POST" && event.path === "/login",
  );
  expect(csrfEvent[0].status).toBe(403);
  expect(csrfEvent[0].origin_class).toBe("same_origin");
  expect(csrfEvent[0].route_calls).toBe(bootstrapEvent[0].route_calls);
  expect(csrfEvent[0].verifier_calls).toBe(bootstrapEvent[0].verifier_calls);
  expect(csrfEvent[0].claim_committed).toBe(bootstrapEvent[0].claim_committed);
  expect(csrfEvent[0].ready_rows).toBe(bootstrapEvent[0].ready_rows);
  const cookieAfterRejection = (await context.cookies(appOrigin)).find((cookie) => cookie.name === "__Host-prelogin");
  expect(cookieAfterRejection?.value === originalCookieSnapshot.value).toBe(true);

  const firstLogin = await page.evaluate(async (csrf) => {
    const response = await fetch("/login", {
      method: "POST",
      credentials: "same-origin",
      headers: {
        Authorization: "Bearer owned",
        "Content-Type": "application/json",
        "X-CSRF-Token": csrf,
      },
      body: "{}",
    });
    return {
      status: response.status,
      cacheControl: response.headers.get("cache-control"),
      vary: response.headers.get("vary"),
    };
  }, csrfToken);
  expect(firstLogin).toEqual({
    status: 200,
    cacheControl: "no-store",
    vary: "Authorization, Content-Type, Cookie, Origin, X-CSRF-Token",
  });
  const successEvent = await waitForNew(csrfEvent[0].seq, 1, (event) =>
    event.fixture === "login" && event.method === "POST" && event.path === "/login",
  );
  expect(successEvent[0].status).toBe(200);
  expect(successEvent[0].origin_class).toBe("same_origin");
  expect(successEvent[0].original_prelogin_unexpired).toBe(true);
  expect(successEvent[0].claim_committed).toBe(true);
  expect(successEvent[0].verifier_calls).toBe((csrfEvent[0].verifier_calls ?? 0) + 1);
  expect(successEvent[0].route_calls).toBe((csrfEvent[0].route_calls ?? 0) + 1);
  expect(successEvent[0].ready_rows).toBe((csrfEvent[0].ready_rows ?? 1) - 1);

  const cookiesAfterLogin = await context.cookies(appOrigin);
  expect(cookiesAfterLogin.some((cookie) => cookie.name === "__Host-prelogin")).toBe(false);
  const authCookie = cookiesAfterLogin.find((cookie) => cookie.name === "__Host-auth");
  expect(authCookie).toBeDefined();
  expect(authCookie?.domain).toBe("localhost");
  expect(authCookie?.path).toBe("/");
  expect(authCookie?.secure).toBe(true);
  expect(authCookie?.httpOnly).toBe(true);
  expect(authCookie?.sameSite).toBe("Strict");

  const replayContext = await browser.newContext({ ignoreHTTPSErrors: false });
  try {
    const replayPage = await replayContext.newPage();
    const replayDocument = await replayPage.goto(`${appOrigin}/`);
    expect(replayDocument?.status()).toBe(200);
    expect(new URL(replayPage.url()).protocol).toBe("https:");
    await replayContext.addCookies([{
      name: originalCookieSnapshot.name,
      value: originalCookieSnapshot.value,
      domain: originalCookieSnapshot.domain,
      path: originalCookieSnapshot.path,
      expires: originalCookieSnapshot.expires > 0 ? originalCookieSnapshot.expires : undefined,
      httpOnly: originalCookieSnapshot.httpOnly,
      secure: originalCookieSnapshot.secure,
      sameSite: originalCookieSnapshot.sameSite,
    }]);
    const replayCookies = await replayContext.cookies(appOrigin);
    const replayCookie = replayCookies.find((cookie) => cookie.name === "__Host-prelogin");
    expect(replayCookies.map((cookie) => cookie.name)).toEqual(["__Host-prelogin"]);
    expect(replayCookie?.value === originalCookieSnapshot.value).toBe(true);
    expect(replayCookies.some((cookie) => cookie.name === "__Host-auth")).toBe(false);

    const replay = await replayPage.evaluate(async (csrf) => {
      const response = await fetch("/login", {
        method: "POST",
        credentials: "same-origin",
        headers: {
          Authorization: "Bearer owned",
          "Content-Type": "application/json",
          "X-CSRF-Token": csrf,
        },
        body: "{}",
      });
      return {
        status: response.status,
        cacheControl: response.headers.get("cache-control"),
        vary: response.headers.get("vary"),
      };
    }, csrfToken);
    expect(replay).toEqual({
      status: 403,
      cacheControl: "no-store",
      vary: "Authorization, Content-Type, Cookie, Origin, X-CSRF-Token",
    });
    const replayEvent = await waitForNew(successEvent[0].seq, 1, (event) =>
      event.fixture === "login" && event.method === "POST" && event.path === "/login",
    );
    expect(replayEvent[0].status).toBe(403);
    expect(replayEvent[0].origin_class).toBe("same_origin");
    expect(replayEvent[0].original_prelogin_unexpired).toBe(true);
    expect(replayEvent[0].claim_committed).toBe(successEvent[0].claim_committed);
    expect(replayEvent[0].verifier_calls).toBe(successEvent[0].verifier_calls);
    expect(replayEvent[0].route_calls).toBe(successEvent[0].route_calls);
    expect(replayEvent[0].ready_rows).toBe(successEvent[0].ready_rows);
  } finally {
    await replayContext.close();
  }
});

test("browser enforces allowed credentialed CORS preflight and response access", async ({ page }) => {
  await page.goto(`${uiOrigin}/`);
  const before = await readObservations();
  const beforeSeq = lastSequence(before);
  const countersBefore = latestFor(before, "cors");

  const allowedRead = await page.evaluate(async (app) => {
    const response = await fetch(`${app}/items/7`, { credentials: "include" });
    return { status: response.status, body: await response.text() };
  }, appOrigin);
  expect(allowedRead).toEqual({ status: 200, body: "owned" });
  const readEvent = await waitForNew(beforeSeq, 1, (event) =>
    event.fixture === "cors" && event.method === "GET" && event.path === "/items/7",
  );
  expect(readEvent[0].status).toBe(200);
  expect(readEvent[0].origin_class).toBe("allowed");
  expect(readEvent[0].allow_origin_present).toBe(true);
  expect(readEvent[0].route_calls).toBe((countersBefore?.route_calls ?? 0) + 1);

  const beforeWrite = await readObservations();
  const beforeWriteSeq = lastSequence(beforeWrite);
  const writeStatus = await page.evaluate(async (app) => {
    const response = await fetch(`${app}/write`, {
      method: "POST",
      credentials: "include",
      headers: { "Content-Type": "application/json", "X-CSRF-Token": "owned" },
      body: "{}",
    });
    return response.status;
  }, appOrigin);
  expect(writeStatus).toBe(200);
  const writeEvents = await waitForNew(beforeWriteSeq, 2, (event) =>
    event.fixture === "cors" && event.path === "/write",
  );
  expect(writeEvents[0].method).toBe("OPTIONS");
  expect(writeEvents[0].status).toBe(204);
  expect(writeEvents[0].origin_class).toBe("allowed");
  expect(writeEvents[0].allow_origin_present).toBe(true);
  expect(writeEvents[0].route_calls).toBe(readEvent[0].route_calls);
  expect(writeEvents[1].method).toBe("POST");
  expect(writeEvents[1].status).toBe(200);
  expect(writeEvents[1].origin_class).toBe("allowed");
  expect(writeEvents[1].allow_origin_present).toBe(true);
  expect(writeEvents[1].route_calls).toBe(readEvent[0].route_calls + 1);
});

test("unapproved owned loopback origin cannot read a CORS response", async ({ page }) => {
  await page.goto(`${deniedOrigin}/`);
  const before = await readObservations();
  const beforeSeq = lastSequence(before);
  const countersBefore = latestFor(before, "cors");
  const browserRead = await page.evaluate(async (app) => {
    try {
      const response = await fetch(`${app}/items/7`, { credentials: "include" });
      return `readable:${response.status}`;
    } catch (error) {
      return error instanceof TypeError ? "blocked-by-cors" : "other-error";
    }
  }, appOrigin);
  expect(browserRead).toBe("blocked-by-cors");
  const observed = await waitForNew(beforeSeq, 1, (event) =>
    event.fixture === "cors" && event.method === "GET" && event.path === "/items/7",
  );
  expect(observed[0].status).toBe(200);
  expect(observed[0].origin_class).toBe("denied");
  expect(observed[0].allow_origin_present).toBe(false);
  expect(observed[0].route_calls).toBe((countersBefore?.route_calls ?? 0) + 1);
});
