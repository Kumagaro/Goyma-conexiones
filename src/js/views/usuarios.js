import { attachDraft, registerDraftOpener } from "../utils/drafts.js";
import { getSession, loadSession } from "../session.js";
import {
  createUser,
  deleteUser,
  listUsers,
  resetUserPassword,
  updateUser,
} from "../services/usersService.js";
import {
  notifyError,
  notifySuccess,
  notifyWarning,
  reportError,
} from "../utils/feedback.js";
import { installFormValidation } from "../utils/formValidation.js";

let userDraft = null;
let users = [];
let editingUserId = null;

function showUserTab(tab) {
  if (tab === "create" && getSession()?.role !== "administrador") return;
  if (tab === "create") void userDraft?.offer();

  document
    .querySelectorAll("#page-usuarios .config-subview")
    .forEach((view) =>
      view.classList.toggle("active", view.id === `usuario-${tab}`),
    );
  document
    .querySelectorAll("#page-usuarios .view-tabs .tab")
    .forEach((button) =>
      button.classList.toggle("active", button.id === `tab-usuario-${tab}`),
    );

  const searchBar = document.getElementById("usuario-search-bar");
  if (searchBar) searchBar.style.display = tab === "list" ? "flex" : "none";
}

export function applyRoleVisibility() {
  const usersMenuItem = document
    .querySelector('a[data-page="usuarios"]')
    ?.closest("li");
  if (!usersMenuItem) return;

  usersMenuItem.style.display =
    getSession()?.role === "administrador" ? "" : "none";
}

let initPromise;

export function initUsersView() {
  if (!initPromise) initPromise = initializeUsersView();
  return initPromise;
}

async function initializeUsersView() {
  await loadSession();
  applyRoleVisibility();

  const createTab = document.getElementById("tab-usuario-create");
  if (createTab && getSession()?.role !== "administrador") {
    createTab.hidden = true;
  }

  document
    .getElementById("tab-usuario-list")
    ?.addEventListener("click", () => showUserTab("list"));
  document
    .getElementById("tab-usuario-create")
    ?.addEventListener("click", () => showUserTab("create"));
  document
    .querySelector("[data-user-action='cancel']")
    ?.addEventListener("click", () => {
      userDraft?.clear();
      resetUserForm();
      showUserTab("list");
    });
  const userForm = document.getElementById("usuario-form");
  if (userForm) {
    installFormValidation(userForm);
    userDraft = attachDraft(userForm, {
      formKey: "user",
      recordId: () => editingUserId,
    });
    userForm.addEventListener("submit", handleCreateUser);
  }

  const userTableBody = document.getElementById("usuarios-body");
  if (userTableBody && getSession()?.role === "administrador") {
    await loadUsers();
    userTableBody.addEventListener("click", handleUserTableAction);
    document
      .getElementById("usuario-search")
      ?.addEventListener("input", handleUserSearch);
  }
}

async function loadUsers() {
  try {
    users = await listUsers();
    renderUserList(users);
  } catch {
    notifyError(null, "No se pudieron cargar los usuarios.");
  }
}

function handleUserSearch(event) {
  const query = event.currentTarget.value.trim().toLowerCase();
  const filteredUsers = users.filter((user) =>
    [user.username, user.phone, user.nationalId].some((value) =>
      value.toLowerCase().includes(query),
    ),
  );
  renderUserList(filteredUsers);
}

function renderUserList(userList) {
  const tableBody = document.getElementById("usuarios-body");
  if (!tableBody) return;

  tableBody.replaceChildren();
  for (const user of userList) {
    const row = document.createElement("tr");
    row.dataset.username = user.username;
    const roleLabel = `${user.role[0].toUpperCase()}${user.role.slice(1)}`;

    for (const value of [
      user.username,
      roleLabel,
      user.phone,
      user.nationalId,
    ]) {
      const cell = document.createElement("td");
      cell.textContent = value;
      row.append(cell);
    }

    const actionsCell = document.createElement("td");
    if (!user.isCurrentUser) {
      const editButton = document.createElement("button");
      editButton.type = "button";
      editButton.className = "btn-secondary";
      editButton.dataset.userAction = "edit";
      editButton.dataset.userId = String(user.id);
      editButton.setAttribute("aria-label", `Editar ${user.username}`);
      editButton.title = `Editar ${user.username}`;
      editButton.innerHTML =
        '<svg class="icon" aria-hidden="true"><use href="#icon-edit"></use></svg>';
      actionsCell.append(editButton);

      const deleteButton = document.createElement("button");
      deleteButton.type = "button";
      deleteButton.className = "btn-danger";
      deleteButton.dataset.userAction = "delete";
      deleteButton.dataset.userId = String(user.id);
      deleteButton.setAttribute("aria-label", `Eliminar ${user.username}`);
      deleteButton.title = `Eliminar ${user.username}`;
      deleteButton.innerHTML =
        '<svg class="icon" aria-hidden="true"><use href="#icon-trash"></use></svg>';
      actionsCell.append(deleteButton);
    }
    row.append(actionsCell);
    tableBody.append(row);
  }
}

async function handleCreateUser(event) {
  event.preventDefault();
  if (getSession()?.role !== "administrador") return;
  if (editingUserId !== null) {
    await handleUpdateUser(event);
    return;
  }

  const form = event.currentTarget;
  const username = form.elements.namedItem("username").value.trim();
  const role = form.elements.namedItem("role").value;
  const phone = form.elements.namedItem("phone").value.trim();
  const nationalId = form.elements.namedItem("nationalId").value.trim();
  const password = form.elements.namedItem("password").value;
  const confirmation = form.elements.namedItem("confirmPassword");
  confirmation.setCustomValidity(
    password === confirmation.value ? "" : "Las contraseñas no coinciden",
  );

  if (!form.checkValidity()) {
    form.reportValidity();
    return;
  }

  try {
    const created = await createUser(
      username,
      role,
      phone,
      nationalId,
      password,
    );
    notifySuccess(`Usuario ${created.username} creado correctamente.`);
    userDraft?.clear();
    form.reset();
  } catch (error) {
    reportError(error, "No se pudo crear el usuario.", USER_FIELDS);
  }
}

function handleUserTableAction(event) {
  const editButton = event.target.closest("button[data-user-action='edit']");
  if (editButton) {
    openEditUser(Number(editButton.dataset.userId));
    return;
  }

  const deleteButton = event.target.closest(
    "button[data-user-action='delete']",
  );
  if (deleteButton) {
    handleDeleteUser(Number(deleteButton.dataset.userId));
  }
}

async function handleDeleteUser(userId) {
  if (getSession()?.role !== "administrador") return;

  const user = users.find((item) => item.id === userId);
  if (!user || user.isCurrentUser) return;

  const confirmed = confirm(
    `¿Estás seguro de que quieres eliminar al usuario "${user.username}"? Esta acción no se puede deshacer.`,
  );
  if (!confirmed) return;

  try {
    await deleteUser(userId);
    users = users.filter((item) => item.id !== userId);
    renderUserList(users);
    notifySuccess(`Usuario ${user.username} eliminado correctamente.`);
  } catch (error) {
    notifyError(error, "No se pudo eliminar el usuario.");
  }
}

function openEditUser(userId) {
  const user = users.find((item) => item.id === userId);
  if (!user || user.isCurrentUser || getSession()?.role !== "administrador") {
    return;
  }

  editingUserId = user.id;
  const form = document.getElementById("usuario-form");
  if (!form) return;
  form.elements.namedItem("username").value = user.username;
  form.elements.namedItem("role").value = user.role;
  form.elements.namedItem("phone").value = user.phone;
  form.elements.namedItem("nationalId").value = user.nationalId;
  form.elements.namedItem("password").value = "";
  form.elements.namedItem("confirmPassword").value = "";
  setEditPasswordMode(true);
  const title = document.getElementById("usuario-form-title");
  if (title) title.textContent = "Editar usuario";
  showUserTab("create");
}

async function handleUpdateUser(event) {
  event.preventDefault();
  if (getSession()?.role !== "administrador" || editingUserId === null) return;

  const form = event.currentTarget;
  const username = form.elements.namedItem("username").value.trim();
  const role = form.elements.namedItem("role").value;
  const phone = form.elements.namedItem("phone").value.trim();
  const nationalId = form.elements.namedItem("nationalId").value.trim();
  const newPassword = form.elements.namedItem("password").value;
  const confirmation = form.elements.namedItem("confirmPassword");
  confirmation.setCustomValidity(
    (newPassword || confirmation.value) && newPassword !== confirmation.value
      ? "Las contraseñas no coinciden"
      : "",
  );
  if (!form.checkValidity()) {
    form.reportValidity();
    return;
  }

  try {
    const updatedUser = await updateUser(
      editingUserId,
      username,
      role,
      phone,
      nationalId,
    );
    users = users.map((user) =>
      user.id === updatedUser.id ? updatedUser : user,
    );
    renderUserList(users);

    if (newPassword) {
      try {
        await resetUserPassword(editingUserId, newPassword);
      } catch (error) {
        notifyWarning(
          `Los datos se guardaron, pero la contraseña no se cambió. ${error?.message || "No se pudo cambiar la contraseña."}`,
        );
        userDraft?.clear();
        resetUserForm();
        showUserTab("list");
        return;
      }
    }

    notifySuccess("Los datos del usuario se actualizaron correctamente.");
    userDraft?.clear();
    resetUserForm();
    showUserTab("list");
  } catch (error) {
    reportError(error, "No se pudo actualizar el usuario.", USER_FIELDS);
  }
}

// Campo al que se refiere cada rechazo de validación del backend (TO-01).
const USER_FIELDS = [
  [/nombre de usuario/i, "usuario-username"],
  [/teléfono/i, "usuario-telefono"],
  [/carnet/i, "usuario-ci"],
  [/contraseña/i, "usuario-password"],
];

function resetUserForm() {
  const form = document.getElementById("usuario-form");
  if (!form) return;
  form.reset();
  setEditPasswordMode(false);
  editingUserId = null;
  const title = document.getElementById("usuario-form-title");
  if (title) title.textContent = "Datos de Usuario";
}

registerDraftOpener("user", (userId) => {
  window.showPage?.(
    "usuarios",
    document.querySelector('#sidebar [data-page="usuarios"]'),
  );
  if (userId == null) showUserTab("create");
  else openEditUser(userId);
});

function setEditPasswordMode(isEditing) {
  const form = document.getElementById("usuario-form");
  if (!form) return;
  form.elements.namedItem("password").required = !isEditing;
  form.elements.namedItem("confirmPassword").required = !isEditing;
  const passwordLabels = form.querySelectorAll(
    "#usuario-password-section .field label",
  );
  if (isEditing) {
    if (passwordLabels[0]) {
      passwordLabels[0].textContent = "Nueva contraseña (opcional)";
    }
    if (passwordLabels[1]) {
      passwordLabels[1].textContent = "Confirmar nueva contraseña";
    }
  } else {
    if (passwordLabels[0]) {
      passwordLabels[0].innerHTML =
        'Contraseña <span class="required">*</span>';
    }
    if (passwordLabels[1]) {
      passwordLabels[1].innerHTML =
        'Confirmar Contraseña <span class="required">*</span>';
    }
  }
}

void initUsersView();
