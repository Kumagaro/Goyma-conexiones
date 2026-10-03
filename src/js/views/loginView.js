import { getCurrentSession, login } from "../services/authService.js";
import { installFormValidation } from "../utils/formValidation.js";

const LOGIN_ERROR_MESSAGE = "Ocurrió un error inesperado";
const SESSION_EXPIRED_MESSAGE =
  "Tu sesión ya no es válida. Inicia sesión de nuevo";
let initPromise;
let dashboardRedirect = (destination) => window.location.replace(destination);

export function setDashboardRedirect(redirect) {
  dashboardRedirect = redirect;
}

export function initLoginView() {
  if (!initPromise) initPromise = initializeLoginView();
  return initPromise;
}

function initializeLoginView() {
  const form = document.getElementById("loginForm");
  if (!form) return;

  const feedback = document.getElementById("loginFeedback");
  if (
    new URLSearchParams(window.location.search).get("reason") === "expired" &&
    feedback
  ) {
    feedback.textContent = SESSION_EXPIRED_MESSAGE;
  }

  installFormValidation(form);
  form.addEventListener("submit", handleLoginSubmit);
  form.hidden = true;

  return getCurrentSession()
    .then((session) => {
      if (session) {
        dashboardRedirect("./dashboard-unificado.html");
        return;
      }
      form.hidden = false;
    })
    .catch(() => {
      form.hidden = false;
    });
}

async function handleLoginSubmit(event) {
  event.preventDefault();

  const form = event.currentTarget;
  if (!form.checkValidity()) {
    form.reportValidity();
    return;
  }

  const username = document.getElementById("username").value;
  const password = document.getElementById("password").value;
  const rememberSession = document.getElementById("rememberSession").checked;
  const feedback = document.getElementById("loginFeedback");
  const submitButton = document.getElementById("loginButton");

  if (feedback) feedback.textContent = "";
  if (submitButton) submitButton.disabled = true;

  try {
    const result = await login(username, password, rememberSession);
    if (result?.code) {
      if (feedback)
        feedback.textContent = result.message || LOGIN_ERROR_MESSAGE;
      return;
    }
    dashboardRedirect("./dashboard-unificado.html");
  } catch (error) {
    if (feedback) {
      feedback.textContent =
        typeof error?.message === "string"
          ? error.message
          : LOGIN_ERROR_MESSAGE;
    }
  } finally {
    if (submitButton) submitButton.disabled = false;
  }
}

void initLoginView();
