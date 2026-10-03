import {
  getSession,
  loadSession,
  onSessionChange,
  refreshSession,
} from "../session.js";
import {
  changeOwnPassword,
  getUserProfile,
  updateOwnProfile,
} from "../services/usersService.js";
import { installFormValidation } from "../utils/formValidation.js";

let profile = null;

function roleLabel(role) {
  return role === "administrador" ? "Administrador" : "Contador";
}

function renderProfileData(userProfile = profile) {
  if (!userProfile) return;
  profile = userProfile;

  const displayRole = roleLabel(userProfile.role);
  const usernameInput = document.getElementById("perfil-username");
  const roleInput = document.getElementById("perfil-role");
  const phoneInput = document.getElementById("perfil-telefono");
  const nationalIdInput = document.getElementById("perfil-national-id");
  const modalUsername = document.getElementById("verperfil-username");
  const modalRole = document.getElementById("verperfil-rol");
  const modalRoleBadge = document.getElementById("verperfil-rol-badge");
  const modalPhone = document.getElementById("verperfil-telefono");
  const modalNationalId = document.getElementById("verperfil-national-id");

  if (usernameInput) usernameInput.value = userProfile.username;
  if (roleInput) roleInput.value = displayRole;
  if (phoneInput) phoneInput.value = userProfile.phone;
  if (nationalIdInput) nationalIdInput.value = userProfile.nationalId;
  if (modalUsername) modalUsername.textContent = userProfile.username;
  if (modalRole) modalRole.textContent = displayRole;
  if (modalRoleBadge) modalRoleBadge.textContent = displayRole;
  if (modalPhone) modalPhone.textContent = userProfile.phone;
  if (modalNationalId) modalNationalId.textContent = userProfile.nationalId;

  renderReservedDataWarning(userProfile.hasReservedContactData);
}

function renderProfileSession(session = getSession()) {
  if (!session || !profile) return;
  renderProfileData({
    ...profile,
    username: session.username,
    role: session.role,
  });
}

function renderReservedDataWarning(hasReservedContactData) {
  document
    .getElementById("reserved-data-warning")
    ?.classList.toggle("is-visible", hasReservedContactData);
}

function showProfileFeedback(message) {
  const feedback = document.getElementById("perfil-feedback");
  if (!feedback) return;
  feedback.textContent = message;
  feedback.classList.add("is-visible");
}

function clearProfileFeedback() {
  const feedback = document.getElementById("perfil-feedback");
  if (!feedback) return;
  feedback.textContent = "";
  feedback.classList.remove("is-visible");
}

function closeProfileModal() {
  document.getElementById("modal-ver-perfil")?.classList.remove("active");
}

function openProfileModal() {
  renderProfileData();
  document.getElementById("modal-ver-perfil")?.classList.add("active");
}

function handleProfileButtonKeydown(event) {
  if (event.key === "Enter" || event.key === " ") {
    event.preventDefault();
    openProfileModal();
  }
}

function openProfilePage() {
  closeProfileModal();
  const profileLink = document.querySelector('#sidebar a[data-page="perfil"]');
  if (profileLink) window.showPage?.("perfil", profileLink);
}

async function handleUpdateOwnProfile(event) {
  event.preventDefault();
  const form = event.currentTarget;
  if (!form.checkValidity()) {
    form.reportValidity();
    return;
  }

  try {
    const updatedProfile = await updateOwnProfile(
      form.elements.namedItem("username").value.trim(),
      form.elements.namedItem("phone").value.trim(),
      form.elements.namedItem("nationalId").value.trim(),
    );
    renderProfileData(updatedProfile);
    clearProfileFeedback();
    await refreshSession();
    showProfileFeedback("Tus datos se actualizaron correctamente.");
  } catch (error) {
    showProfileFeedback(error?.message || "No se pudo actualizar tu perfil.");
  }
}

async function handleChangeOwnPassword(event) {
  event.preventDefault();
  const form = event.currentTarget;
  const currentPassword = form.elements.namedItem("currentPassword");
  const newPassword = form.elements.namedItem("newPassword");
  const confirmation = form.elements.namedItem("confirmPassword");

  confirmation.setCustomValidity(
    newPassword.value === confirmation.value
      ? ""
      : "Las contraseñas no coinciden",
  );
  if (!form.checkValidity()) {
    form.reportValidity();
    return;
  }

  try {
    await changeOwnPassword(currentPassword.value, newPassword.value);
    form.reset();
    clearProfileFeedback();
    showProfileFeedback("La contraseña se cambió correctamente.");
  } catch (error) {
    showProfileFeedback(error?.message || "No se pudo cambiar la contraseña.");
  }
}

let initPromise;

export function initProfileView() {
  if (!initPromise) initPromise = initializeProfileView();
  return initPromise;
}

async function initializeProfileView() {
  const session = await loadSession();
  if (!session) return;

  try {
    renderProfileData(await getUserProfile());
  } catch {
    showProfileFeedback("No se pudo cargar tu perfil.");
  }

  const profileButton = document.querySelector(".usuario");
  profileButton?.addEventListener("click", openProfileModal);
  profileButton?.addEventListener("keydown", handleProfileButtonKeydown);
  document
    .getElementById("modal-ver-perfil")
    ?.addEventListener("click", (event) => {
      if (event.target.id === "modal-ver-perfil") closeProfileModal();
    });

  document
    .querySelectorAll("[data-profile-action='close']")
    .forEach((button) => button.addEventListener("click", closeProfileModal));
  document
    .querySelector("[data-profile-action='edit']")
    ?.addEventListener("click", openProfilePage);
  const profileForm = document.getElementById("profile-form");
  if (profileForm) {
    installFormValidation(profileForm);
    profileForm.addEventListener("submit", handleUpdateOwnProfile);
  }
  const passwordForm = document.getElementById("profile-password-form");
  if (passwordForm) {
    installFormValidation(passwordForm);
    passwordForm.addEventListener("submit", handleChangeOwnPassword);
  }

  document.addEventListener("keydown", (event) => {
    if (event.key === "Escape") closeProfileModal();
  });
  onSessionChange(renderProfileSession);
}

void initProfileView();
