import { getCurrentSession, login } from "../services/authService.js";
import { notifyError, notifyWarning } from "../utils/feedback.js";
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

  if (new URLSearchParams(window.location.search).get("reason") === "expired") {
    notifyWarning(SESSION_EXPIRED_MESSAGE);
  }

  installFormValidation(form);
  bindPasswordToggle();
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

// Muestra u oculta la contraseña (antes, un script inline de index.html).
function bindPasswordToggle() {
  const toggle = document.getElementById("togglePassword");
  const passwordInput = document.getElementById("password");
  if (!toggle || !passwordInput) return;
  toggle.addEventListener("click", () => {
    const isHidden = passwordInput.type === "password";
    passwordInput.type = isHidden ? "text" : "password";
    toggle
      .querySelector("use")
      ?.setAttribute("href", isHidden ? "#icon-eye-off" : "#icon-eye");
    toggle.setAttribute(
      "aria-label",
      isHidden ? "Ocultar contraseña" : "Mostrar contraseña",
    );
  });
  toggle.addEventListener("keydown", (event) => {
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      toggle.click();
    }
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
  const submitButton = document.getElementById("loginButton");

  if (submitButton) submitButton.disabled = true;

  try {
    const result = await login(username, password, rememberSession);
    if (result?.code) {
      notifyError(result, LOGIN_ERROR_MESSAGE);
      return;
    }
    dashboardRedirect("./dashboard-unificado.html");
  } catch (error) {
    notifyError(
      typeof error?.message === "string" ? error : null,
      LOGIN_ERROR_MESSAGE,
    );
  } finally {
    if (submitButton) submitButton.disabled = false;
  }
}

void initLoginView();
