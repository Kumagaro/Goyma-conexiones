import { invoke } from "./invoke.js";

export function createUser(username, role, phone, nationalId, password) {
  return invoke("create_user", { username, role, phone, nationalId, password });
}

export function listUsers() {
  return invoke("list_users");
}

export function updateUser(userId, username, role, phone, nationalId) {
  return invoke("update_user", { userId, username, role, phone, nationalId });
}

export function resetUserPassword(userId, newPassword, currentPassword) {
  const args = { userId, newPassword };
  if (currentPassword !== undefined) args.currentPassword = currentPassword;
  return invoke("reset_user_password", args);
}

export function getUserProfile() {
  return invoke("get_user_profile");
}

export function updateOwnProfile(username, phone, nationalId) {
  return invoke("update_own_profile", { username, phone, nationalId });
}

export function changeOwnPassword(currentPassword, newPassword) {
  return invoke("change_own_password", { currentPassword, newPassword });
}

export function deleteUser(userId) {
  return invoke("delete_user", { userId });
}
