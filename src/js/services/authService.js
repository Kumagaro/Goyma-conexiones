import { invoke } from "./invoke.js";

const ACTIVITY_THROTTLE_MS = 30_000;
const DOCUMENT_ACTIVITY_EVENTS = [
  "click",
  "keydown",
  "mousemove",
  "pointermove",
];
const WINDOW_ACTIVITY_EVENTS = ["popstate", "hashchange"];

let lastActivityCall = 0;
let activityListenerStarted = false;
let touchSessionEnabled = false;

export function getSessionInfo() {
  return invoke("get_session_info");
}

export function getCurrentSession() {
  return invoke("get_current_session");
}

export function login(username, password, rememberSession) {
  return invoke("login", { username, password, rememberSession });
}

export function logout() {
  return invoke("logout");
}

export function verifyRecoveryIdentity(username, phone, nationalId) {
  return invoke("verify_recovery_identity", { username, phone, nationalId });
}

export function recoverPassword(username, phone, nationalId, newPassword) {
  return invoke("recover_password", {
    username,
    phone,
    nationalId,
    newPassword,
  });
}

export function startActivityListener() {
  if (activityListenerStarted || typeof document === "undefined") {
    return;
  }
  activityListenerStarted = true;
  for (const eventName of DOCUMENT_ACTIVITY_EVENTS) {
    document.addEventListener(eventName, handleActivity, { passive: true });
  }
  for (const eventName of WINDOW_ACTIVITY_EVENTS) {
    window.addEventListener(eventName, handleActivity, { passive: true });
  }
}

export function enableTouchSession() {
  touchSessionEnabled = true;
}

export function touchSession() {
  return invoke("touch_session");
}

function handleActivity() {
  const now = Date.now();
  if (!touchSessionEnabled || now - lastActivityCall < ACTIVITY_THROTTLE_MS) {
    return;
  }
  lastActivityCall = now;
  touchSession().catch(() => {});
}
