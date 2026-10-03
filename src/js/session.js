import {
  enableTouchSession,
  getSessionInfo,
  startActivityListener,
} from "./services/authService.js";

let session = null;
let loadPromise = null;
let redirectHandler = null;
const subscribers = new Set();

export function loadSession() {
  if (!loadPromise) {
    loadPromise = getSessionInfo()
      .then((result) => {
        if (result?.code === "sessionExpired") {
          session = null;
          return null;
        }
        session = result ?? null;
        if (!session) {
          redirectToLogin();
          return null;
        }
        startActivityListener();
        enableTouchSession();
        return session;
      })
      .catch((error) => {
        session = null;
        if (error?.code !== "sessionExpired") {
          redirectToLogin();
        }
        return null;
      });
  }
  return loadPromise;
}

export function getSession() {
  return session;
}

export async function refreshSession() {
  try {
    const result = await getSessionInfo();
    if (result?.code === "sessionExpired") {
      session = null;
      return null;
    }
    session = result ?? null;
  } catch (error) {
    session = null;
    if (error?.code !== "sessionExpired") {
      redirectToLogin();
    }
    return null;
  }

  if (!session) {
    redirectToLogin();
    return null;
  }

  for (const subscriber of subscribers) {
    subscriber(session);
  }
  return session;
}

export function onSessionChange(callback) {
  subscribers.add(callback);
  return () => subscribers.delete(callback);
}

export function redirectToLogin() {
  const destination = "./index.html";
  if (redirectHandler) {
    redirectHandler(destination);
    return;
  }
  window.location.replace(destination);
}

export function setRedirectToLogin(handler) {
  redirectHandler = handler;
}
